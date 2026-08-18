
use crate::interp::{Runtime, RuntimeError};
use crate::value::{
    FunctionInternals, InternalKind, NativeFn, Object, ObjectRef, PropertyDescriptor, Value,
};
use std::rc::Rc;

pub fn array_iterator_prototype(rt: &mut Runtime) -> ObjectRef {
    match rt.array_iterator_prototype {
        Some(id) => id,
        None => {
            let mut proto = Object::new_ordinary();
            proto.proto = rt.iterator_prototype;
            let id = rt.alloc_object(proto);

            install_iterator_proto_data_with_writable(
                rt,
                id,
                "@@toStringTag",
                Value::String(Rc::new(crate::value::JsString::from("Array Iterator"))),
                false,
            );

            install_self_returning_iterator(rt, id);
            install_next(rt, id, |rt, _args| {
                let it = match rt.current_this() {
                    Value::Object(id) => id,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "array iterator next: this is not an iterator".into(),
                        ))
                    }
                };
                let src =
                    require_own_indexed_iterator_slot(rt, it, "__arr", "array iterator next")?;
                let src_id = match src {
                    Value::Object(id) => id,
                    _ => return Ok(iter_result_done(rt)),
                };
                let i = match require_own_indexed_iterator_slot(
                    rt,
                    it,
                    "__i",
                    "array iterator next",
                )? {
                    Value::Number(n) => n as usize,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "array iterator next: receiver lacks iterator slots".into(),
                        ))
                    }
                };
                if rt.typed_array_view_out_of_bounds(src_id) {
                    return Err(RuntimeError::TypeError(
                        "array iterator next: TypedArray source is out of bounds".into(),
                    ));
                }
                let len = rt
                    .typed_array_view_len(src_id)
                    .unwrap_or_else(|| rt.array_length(src_id));
                if i >= len {
                    rt.set_engine_sentinel(it, "__arr", Value::Undefined);
                    return Ok(iter_result_done(rt));
                }
                let kind = match rt.object_get(it, "__kind") {
                    Value::String(s) => s.as_ref().clone(),
                    _ => "value".into(),
                };
                let v = match kind.as_str() {
                    "key" => Value::Number(i as f64),
                    "entry" => {
                        let pair = rt.alloc_object(Object::new_array());
                        rt.create_data_property_or_throw(
                            &Value::Object(pair),
                            "0",
                            Value::Number(i as f64),
                        )?;
                        let elem = rt.spec_get(&Value::Object(src_id), &i.to_string())?;
                        rt.create_data_property_or_throw(&Value::Object(pair), "1", elem)?;
                        rt.object_set(pair, "length".into(), Value::Number(2.0));
                        Value::Object(pair)
                    }
                    _ => rt.spec_get(&Value::Object(src_id), &i.to_string())?,
                };
                rt.object_set(it, "__i".into(), Value::Number((i + 1) as f64));
                Ok(iter_result_value(rt, v))
            });
            rt.array_iterator_prototype = Some(id);
            if let Some(realm) = rt.realms.get_mut(rt.current_realm) {
                realm.array_iterator_prototype = Some(id);
            }
            id
        }
    }
}

pub fn make_array_iterator(rt: &mut Runtime, src: ObjectRef) -> ObjectRef {
    make_array_iterator_with_kind(rt, src, "value")
}

pub fn make_array_key_iterator(rt: &mut Runtime, src: ObjectRef) -> ObjectRef {
    make_array_iterator_with_kind(rt, src, "key")
}

pub fn make_array_entry_iterator(rt: &mut Runtime, src: ObjectRef) -> ObjectRef {
    make_array_iterator_with_kind(rt, src, "entry")
}

fn make_array_iterator_with_kind(rt: &mut Runtime, src: ObjectRef, kind: &str) -> ObjectRef {
    let array_iter_proto = array_iterator_prototype(rt);
    let iter = make_indexed_iterator_with_proto(rt, src, array_iter_proto);
    rt.set_engine_sentinel(
        iter,
        "__kind",
        Value::String(Rc::new(crate::value::JsString::from(kind))),
    );
    iter
}

pub fn regexp_string_iterator_prototype(rt: &mut Runtime) -> ObjectRef {
    match rt.regexp_string_iterator_prototype {
        Some(id) => id,
        None => {
            let mut proto = Object::new_ordinary();
            proto.proto = rt.iterator_prototype;
            let id = rt.alloc_object(proto);
            install_iterator_proto_data_with_writable(
                rt,
                id,
                "@@toStringTag",
                Value::String(Rc::new(crate::value::JsString::from(
                    "RegExp String Iterator",
                ))),
                false,
            );
            install_next(rt, id, |rt, _args| {
                let it = match rt.current_this() {
                    Value::Object(id) => id,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "RegExp string iterator next: this is not an iterator".into(),
                        ))
                    }
                };
                regexp_string_iterator_next(rt, it)
            });
            rt.regexp_string_iterator_prototype = Some(id);
            if let Some(realm) = rt.realms.get_mut(rt.current_realm) {
                realm.regexp_string_iterator_prototype = Some(id);
            }
            id
        }
    }
}

pub fn make_regexp_string_iterator(
    rt: &mut Runtime,
    regexp: ObjectRef,
    input: &str,
    global: bool,
    unicode: bool,
) -> ObjectRef {
    let proto = regexp_string_iterator_prototype(rt);
    let mut iter_obj = Object::new_ordinary();
    iter_obj.proto = Some(proto);
    let iter = rt.alloc_object(iter_obj);
    rt.set_engine_sentinel(iter, "__regexpStringIteratorRegExp", Value::Object(regexp));
    rt.set_engine_sentinel(
        iter,
        "__regexpStringIteratorString",
        Value::String(Rc::new(crate::value::JsString::from(input.to_string()))),
    );
    rt.set_engine_sentinel(iter, "__regexpStringIteratorGlobal", Value::Boolean(global));
    rt.set_engine_sentinel(
        iter,
        "__regexpStringIteratorUnicode",
        Value::Boolean(unicode),
    );
    rt.set_engine_sentinel(iter, "__regexpStringIteratorDone", Value::Boolean(false));
    iter
}

fn regexp_string_iterator_next(rt: &mut Runtime, it: ObjectRef) -> Result<Value, RuntimeError> {
    let done = require_own_indexed_iterator_slot(
        rt,
        it,
        "__regexpStringIteratorDone",
        "RegExp string iterator next",
    )?;
    if matches!(done, Value::Boolean(true)) {
        return Ok(iter_result_done(rt));
    }
    let regexp = match require_own_indexed_iterator_slot(
        rt,
        it,
        "__regexpStringIteratorRegExp",
        "RegExp string iterator next",
    )? {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp string iterator next: receiver lacks iterator slots".into(),
            ))
        }
    };
    let input = match require_own_indexed_iterator_slot(
        rt,
        it,
        "__regexpStringIteratorString",
        "RegExp string iterator next",
    )? {
        Value::String(s) => s.as_str().to_string(),
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp string iterator next: receiver lacks iterator slots".into(),
            ))
        }
    };
    let global = matches!(
        require_own_indexed_iterator_slot(
            rt,
            it,
            "__regexpStringIteratorGlobal",
            "RegExp string iterator next",
        )?,
        Value::Boolean(true)
    );
    let unicode = matches!(
        require_own_indexed_iterator_slot(
            rt,
            it,
            "__regexpStringIteratorUnicode",
            "RegExp string iterator next",
        )?,
        Value::Boolean(true)
    );

    let result = crate::regexp::regexp_exec_generic_object(rt, regexp, &input)?;
    if matches!(result, Value::Null) {
        rt.set_engine_sentinel(it, "__regexpStringIteratorDone", Value::Boolean(true));
        return Ok(iter_result_done(rt));
    }
    let match_id = match result {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "RegExp exec result is not an Object or null".into(),
            ))
        }
    };
    if !global {
        rt.set_engine_sentinel(it, "__regexpStringIteratorDone", Value::Boolean(true));
        return Ok(iter_result_value(rt, Value::Object(match_id)));
    }

    let matched_v = rt.read_property(match_id, "0")?;
    let matched = rt.coerce_to_string(&matched_v)?;
    if matched.is_empty() {
        let last_index_v = rt.read_property(regexp, "lastIndex")?;
        let current = crate::regexp::regexp_to_length(rt, &last_index_v)? as usize;
        let next = crate::regexp::advance_string_index_utf16(&input, current, unicode) as f64;
        crate::regexp::set_last_index_strict(rt, regexp, next)?;
    }
    Ok(iter_result_value(rt, Value::Object(match_id)))
}

pub fn string_iterator_prototype(rt: &mut Runtime) -> ObjectRef {
    match rt.string_iterator_prototype {
        Some(id) => id,
        None => {
            let mut proto = Object::new_ordinary();
            proto.proto = rt.iterator_prototype;
            let id = rt.alloc_object(proto);
            install_iterator_proto_data_with_writable(
                rt,
                id,
                "@@toStringTag",
                Value::String(Rc::new(crate::value::JsString::from("String Iterator"))),
                false,
            );
            install_next(rt, id, |rt, _args| {
                let it = match rt.current_this() {
                    Value::Object(id) => id,
                    _ => {
                        return Err(RuntimeError::TypeError(
                            "String iterator next: this is not an iterator".into(),
                        ))
                    }
                };
                indexed_iterator_next(rt, it)
            });
            rt.string_iterator_prototype = Some(id);
            if let Some(realm) = rt.realms.get_mut(rt.current_realm) {
                realm.string_iterator_prototype = Some(id);
            }
            id
        }
    }
}

pub fn make_string_indexed_iterator(rt: &mut Runtime, src: ObjectRef) -> ObjectRef {
    let proto = string_iterator_prototype(rt);
    make_indexed_iterator_with_proto(rt, src, proto)
}

fn make_indexed_iterator_with_proto(
    rt: &mut Runtime,
    src: ObjectRef,
    proto: ObjectRef,
) -> ObjectRef {
    let mut iter_obj = Object::new_ordinary();
    iter_obj.proto = Some(proto);
    let iter = rt.alloc_object(iter_obj);
    rt.set_engine_sentinel(iter, "__arr", Value::Object(src));
    rt.set_engine_sentinel(iter, "__i", Value::Number(0.0));
    iter
}

fn indexed_iterator_next(rt: &mut Runtime, it: ObjectRef) -> Result<Value, RuntimeError> {
    let src = require_own_indexed_iterator_slot(rt, it, "__arr", "indexed iterator next")?;
    let src_id = match src {
        Value::Object(id) => id,
        _ => return Ok(iter_result_done(rt)),
    };
    let i = match require_own_indexed_iterator_slot(rt, it, "__i", "indexed iterator next")? {
        Value::Number(n) => n as usize,
        _ => {
            return Err(RuntimeError::TypeError(
                "indexed iterator next: receiver lacks iterator slots".into(),
            ))
        }
    };
    let len = rt.array_length(src_id);
    if i >= len {
        return Ok(iter_result_done(rt));
    }
    let v = rt.object_get(src_id, &i.to_string());
    rt.object_set(it, "__i".into(), Value::Number((i + 1) as f64));
    Ok(iter_result_value(rt, v))
}

fn require_own_indexed_iterator_slot(
    rt: &Runtime,
    it: ObjectRef,
    name: &str,
    label: &str,
) -> Result<Value, RuntimeError> {
    rt.obj(it)
        .get_own(name)
        .map(|d| d.value.clone())
        .ok_or_else(|| RuntimeError::TypeError(format!("{}: receiver lacks iterator slots", label)))
}

pub fn make_string_iterator(rt: &mut Runtime, js: &crate::value::JsString) -> ObjectRef {

    let units = js.code_units();
    let arr = rt.alloc_object(Object::new_array());
    let mut i = 0usize;
    let mut idx = 0usize;
    while i < units.len() {
        let u = units[i];
        let cp_units: Vec<u16> = if (0xD800..=0xDBFF).contains(&u)
            && i + 1 < units.len()
            && (0xDC00..=0xDFFF).contains(&units[i + 1])
        {
            let v = vec![u, units[i + 1]];
            i += 2;
            v
        } else {
            i += 1;
            vec![u]
        };
        rt.object_set(
            arr,
            idx.to_string(),
            Value::String(Rc::new(crate::value::JsString::from_code_units(cp_units))),
        );
        idx += 1;
    }
    rt.object_set(arr, "length".into(), Value::Number(idx as f64));
    make_string_indexed_iterator(rt, arr)
}

fn install_self_returning_iterator(rt: &mut Runtime, host: ObjectRef) {
    let native: NativeFn = Rc::new(|rt, _args| Ok(rt.current_this()));
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, "[Symbol.iterator]", 0.0);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "[Symbol.iterator]".into(),
            length: 0,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);

    install_iterator_proto_data(rt, host, "@@iterator", Value::Object(fn_id));
}

fn install_next<F>(rt: &mut Runtime, host: ObjectRef, f: F)
where
    F: Fn(&mut Runtime, &[Value]) -> Result<Value, RuntimeError> + 'static,
{
    let native: NativeFn = Rc::new(f);
    let mut properties = indexmap::IndexMap::new();
    crate::value::install_function_meta_props(&mut properties, "next", 0.0);
    let fn_obj = Object {
        proto: None,
        extensible: true,
        properties,
        internal_kind: InternalKind::Function(Box::new(FunctionInternals {
            name: "next".into(),
            length: 0,
            native,
            is_constructor: false,
            creation_realm: 0,
            roots: Vec::new(),
        })),

        ..Default::default()
    };
    let fn_id = rt.alloc_object(fn_obj);

    install_iterator_proto_data(rt, host, "next", Value::Object(fn_id));
}

fn install_iterator_proto_data(rt: &mut Runtime, host: ObjectRef, name: &str, value: Value) {
    install_iterator_proto_data_with_writable(rt, host, name, value, true);
}

fn install_iterator_proto_data_with_writable(
    rt: &mut Runtime,
    host: ObjectRef,
    name: &str,
    value: Value,
    writable: bool,
) {
    rt.obj_mut(host).dict_mut().insert(
        name.into(),
        PropertyDescriptor {
            value: value.clone(),
            writable,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    if name == "@@iterator" {
        if let Value::Object(symbol_ctor) = rt.global_get("Symbol") {
            if let Value::Symbol(iterator_symbol) = rt.object_get(symbol_ctor, "iterator") {
                rt.obj_mut(host).dict_mut().insert(
                    crate::value::PropertyKey::Symbol(iterator_symbol),
                    PropertyDescriptor {
                        value,
                        writable,
                        enumerable: false,
                        configurable: true,
                        getter: None,
                        setter: None,
                    },
                );
            }
        }
    }
}

pub fn iter_result_value(rt: &mut Runtime, v: Value) -> Value {
    let id = rt.alloc_object(Object::new_ordinary());
    rt.obj_mut(id).dict_mut().insert(
        "value".into(),
        PropertyDescriptor {
            value: v,
            writable: true,
            enumerable: true,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    rt.obj_mut(id).dict_mut().insert(
        "done".into(),
        PropertyDescriptor {
            value: Value::Boolean(false),
            writable: true,
            enumerable: true,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    Value::Object(id)
}

pub fn iter_result_done(rt: &mut Runtime) -> Value {
    let id = rt.alloc_object(Object::new_ordinary());
    rt.obj_mut(id).dict_mut().insert(
        "value".into(),
        PropertyDescriptor {
            value: Value::Undefined,
            writable: true,
            enumerable: true,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    rt.obj_mut(id).dict_mut().insert(
        "done".into(),
        PropertyDescriptor {
            value: Value::Boolean(true),
            writable: true,
            enumerable: true,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    Value::Object(id)
}

fn decode_map_storage_key(rt: &mut Runtime, target: ObjectRef, k: &str) -> Value {

    if let Value::Object(orig) = rt.object_get(target, "__map_orig_keys") {
        let v = rt.object_get(orig, k);
        if !matches!(v, Value::Undefined) {
            return v;
        }
    }
    Value::String(Rc::new(crate::value::JsString::from(k.to_string())))
}

fn collection_iterator_next(rt: &mut Runtime, _args: &[Value]) -> Result<Value, RuntimeError> {
    let it = match rt.current_this() {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "Collection Iterator.next: this is not an iterator".into(),
            ))
        }
    };
    let target = match rt.object_get(it, "__coll_target") {
        Value::Object(id) => id,
        _ => {
            return Err(RuntimeError::TypeError(
                "Collection Iterator.next: receiver lacks collection iterator slots".into(),
            ))
        }
    };
    let storage_key = match rt.object_get(it, "__coll_storage") {
        Value::String(s) => (*s).to_string(),
        _ => {
            return Err(RuntimeError::TypeError(
                "Collection Iterator.next: receiver lacks collection iterator slots".into(),
            ))
        }
    };
    if !matches!(rt.object_get(it, "__coll_kind"), Value::String(_)) {
        return Err(RuntimeError::TypeError(
            "Collection Iterator.next: receiver lacks collection iterator slots".into(),
        ));
    }
    if !matches!(rt.object_get(it, "__coll_index"), Value::Number(_)) {
        return Err(RuntimeError::TypeError(
            "Collection Iterator.next: receiver lacks collection iterator slots".into(),
        ));
    }
    if matches!(rt.object_get(it, "__coll_done"), Value::Undefined) {
        return Err(RuntimeError::TypeError(
            "Collection Iterator.next: receiver lacks collection iterator slots".into(),
        ));
    }
    if crate::abstract_ops::to_boolean(&rt.object_get(it, "__coll_done")) {
        return Ok(iter_result_done(rt));
    }
    let storage = match rt.object_get(target, &storage_key) {
        Value::Object(id) => id,
        _ => return Ok(iter_result_done(rt)),
    };
    let idx = match rt.object_get(it, "__coll_index") {
        Value::Number(n) => n as usize,
        _ => 0,
    };
    let key_s: Option<String> = rt
        .obj(storage)
        .properties
        .iter()
        .nth(idx)
        .map(|(k, _)| k.to_string_content());
    let key_s = match key_s {
        Some(k) => k,
        None => {
            rt.set_engine_sentinel(it, "__coll_done", Value::Boolean(true));
            return Ok(iter_result_done(rt));
        }
    };
    let value = rt.object_get(storage, &key_s);
    rt.set_engine_sentinel(it, "__coll_index", Value::Number((idx + 1) as f64));
    let kind = match rt.object_get(it, "__coll_kind") {
        Value::String(s) => (*s).to_string(),
        _ => "entry".to_string(),
    };
    let is_map = storage_key == "__map_data";
    let key_val = if is_map {
        decode_map_storage_key(rt, target, &key_s)
    } else {
        value.clone()
    };
    let result_val = match kind.as_str() {
        "key" => key_val,
        "value" => value,
        _ => {
            let pair = rt.alloc_object(Object::new_array());
            rt.object_set(pair, "0".into(), key_val);
            rt.object_set(pair, "1".into(), value);
            rt.object_set(pair, "length".into(), Value::Number(2.0));
            Value::Object(pair)
        }
    };
    Ok(iter_result_value(rt, result_val))
}

fn collection_iterator_prototype(rt: &mut Runtime, cache_key: &str, tag: &str) -> ObjectRef {
    if let Some(Value::Object(id)) = rt.engine_helpers.get(cache_key).cloned() {
        return id;
    }
    let mut proto = Object::new_ordinary();
    proto.proto = rt.iterator_prototype.or(rt.object_prototype);
    let proto_id = rt.alloc_object(proto);

    rt.obj_mut(proto_id).dict_mut().insert(
        crate::value::PropertyKey::String("@@toStringTag".into()),
        PropertyDescriptor {
            value: Value::String(Rc::new(crate::value::JsString::from(tag.to_string()))),
            writable: false,
            enumerable: false,
            configurable: true,
            getter: None,
            setter: None,
        },
    );
    install_self_returning_iterator(rt, proto_id);
    install_next(rt, proto_id, collection_iterator_next);
    rt.engine_helpers
        .insert(cache_key.to_string(), Value::Object(proto_id));
    proto_id
}

fn make_collection_iterator(
    rt: &mut Runtime,
    target: ObjectRef,
    storage_key: &str,
    kind: &str,
    cache_key: &str,
    tag: &str,
) -> ObjectRef {
    let proto = collection_iterator_prototype(rt, cache_key, tag);
    let mut o = Object::new_ordinary();
    o.proto = Some(proto);
    let it = rt.alloc_object(o);
    rt.set_engine_sentinel(it, "__coll_target", Value::Object(target));
    rt.set_engine_sentinel(
        it,
        "__coll_storage",
        Value::String(Rc::new(crate::value::JsString::from(
            storage_key.to_string(),
        ))),
    );
    rt.set_engine_sentinel(
        it,
        "__coll_kind",
        Value::String(Rc::new(crate::value::JsString::from(kind.to_string()))),
    );
    rt.set_engine_sentinel(it, "__coll_index", Value::Number(0.0));
    rt.set_engine_sentinel(it, "__coll_done", Value::Boolean(false));
    it
}

pub fn make_map_iterator(rt: &mut Runtime, map_id: ObjectRef, kind: &str) -> ObjectRef {
    make_collection_iterator(
        rt,
        map_id,
        "__map_data",
        kind,
        "__MapIteratorPrototype",
        "Map Iterator",
    )
}

pub fn make_set_iterator(rt: &mut Runtime, set_id: ObjectRef, kind: &str) -> ObjectRef {
    make_collection_iterator(
        rt,
        set_id,
        "__set_data",
        kind,
        "__SetIteratorPrototype",
        "Set Iterator",
    )
}
