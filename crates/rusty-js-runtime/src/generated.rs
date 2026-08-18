
use crate::interp::{Runtime, RuntimeError};
use crate::value::Value;

pub fn array_prototype_map(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let callbackfn = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&callbackfn.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.map: callback is not callable".into(),
        ));
    }

    let a = rt.array_species_create(&o.clone(), len.clone())?;
    let _a_root_guard = rt.push_temporary_value_roots(std::slice::from_ref(&a));

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        let k_present = rt.has_property_via_throw(&o.clone(), &pk.clone())?;

        if k_present.clone() {

            let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

            let mapped = {
                let _temporary_value_roots = rt.push_temporary_value_roots(&[
                    a.clone(),
                    o.clone(),
                    callbackfn.clone(),
                    this_arg.clone(),
                    k_value.clone(),
                ]);
                rt.call_function(
                    callbackfn.clone().clone(),
                    this_arg.clone().clone(),
                    vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
                )?
            };

            {
                let _temporary_value_roots =
                    rt.push_temporary_value_roots(&[a.clone(), mapped.clone()]);
                {
                    rt.create_data_property_or_throw(&a.clone(), &pk.clone(), mapped.clone())?;
                    Value::Undefined
                }
            };
        }

        k = k.clone() + 1_usize;
    }

    return Ok(a.clone());
}

pub fn array_prototype_for_each(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let callbackfn = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&callbackfn.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.forEach: callback is not callable".into(),
        ));
    }

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        if rt.has_property_via_throw(&o.clone(), &pk.clone())? {

            let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

            rt.call_function(
                callbackfn.clone().clone(),
                this_arg.clone().clone(),
                vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
            )?;
        }

        k = k.clone() + 1_usize;
    }

    return Ok(Value::Undefined);
}

pub fn array_prototype_filter(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let callbackfn = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&callbackfn.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.filter: callback is not callable".into(),
        ));
    }

    let a = rt.array_species_create(&o.clone(), 0_usize)?;
    let _a_root_guard = rt.push_temporary_value_roots(std::slice::from_ref(&a));

    let mut k: usize = 0_usize;

    let mut to: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        if rt.has_property_via_throw(&o.clone(), &pk.clone())? {

            let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

            let selected = crate::abstract_ops::to_boolean(&{
                let _temporary_value_roots = rt.push_temporary_value_roots(&[
                    a.clone(),
                    o.clone(),
                    callbackfn.clone(),
                    this_arg.clone(),
                    k_value.clone(),
                ]);
                rt.call_function(
                    callbackfn.clone().clone(),
                    this_arg.clone().clone(),
                    vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
                )?
            });

            if selected.clone() {

                {
                    let _temporary_value_roots =
                        rt.push_temporary_value_roots(&[a.clone(), k_value.clone()]);
                    {
                        rt.create_data_property_or_throw(
                            &a.clone(),
                            &to.clone().to_string(),
                            k_value.clone(),
                        )?;
                        Value::Undefined
                    }
                };

                to = to.clone() + 1_usize;
            }
        }

        k = k.clone() + 1_usize;
    }

    return Ok(a.clone());
}

pub fn array_prototype_every(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let callbackfn = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&callbackfn.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.every: callback is not callable".into(),
        ));
    }

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        if rt.has_property_via(&o.clone(), &pk.clone()) {

            let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

            let test_result = crate::abstract_ops::to_boolean(&rt.call_function(
                callbackfn.clone().clone(),
                this_arg.clone().clone(),
                vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
            )?);

            if !test_result.clone() {

                return Ok(Value::Boolean(false));
            }
        }

        k = k.clone() + 1_usize;
    }

    return Ok(Value::Boolean(true));
}

pub fn array_prototype_some(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let callbackfn = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&callbackfn.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.some: callback is not callable".into(),
        ));
    }

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        if rt.has_property_via(&o.clone(), &pk.clone()) {

            let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

            let test_result = crate::abstract_ops::to_boolean(&rt.call_function(
                callbackfn.clone().clone(),
                this_arg.clone().clone(),
                vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
            )?);

            if test_result.clone() {

                return Ok(Value::Boolean(true));
            }
        }

        k = k.clone() + 1_usize;
    }

    return Ok(Value::Boolean(false));
}

pub fn array_prototype_find(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let predicate = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&predicate.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.find: predicate is not callable".into(),
        ));
    }

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

        let test_result = crate::abstract_ops::to_boolean(&rt.call_function(
            predicate.clone().clone(),
            this_arg.clone().clone(),
            vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
        )?);

        if test_result.clone() {

            return Ok(k_value.clone());
        }

        k = k.clone() + 1_usize;
    }

    return Ok(Value::Undefined);
}

pub fn array_prototype_find_index(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let predicate = args.get(0).cloned().unwrap_or(Value::Undefined);

    let this_arg = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&this.clone())?;

    let len: usize = rt.length_of_array_like(&o.clone())?;

    if !rt.is_callable(&predicate.clone()) {

        return Err(RuntimeError::TypeError(
            "Array.prototype.findIndex: predicate is not callable".into(),
        ));
    }

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let pk = k.clone().to_string();

        let k_value = rt.spec_get(&o.clone(), &pk.clone())?;

        let test_result = crate::abstract_ops::to_boolean(&rt.call_function(
            predicate.clone().clone(),
            this_arg.clone().clone(),
            vec![k_value.clone(), Value::Number(k.clone() as f64), o.clone()],
        )?);

        if test_result.clone() {

            return Ok(Value::Number(k.clone() as f64));
        }

        k = k.clone() + 1_usize;
    }

    return Ok(Value::Number(-1_f64));
}

pub fn array_prototype_find_last(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_find_last_via(args)?);
}

pub fn array_prototype_find_last_index(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_find_last_index_via(args)?);
}

pub fn array_prototype_index_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_index_of_via(args)?);
}

pub fn array_prototype_includes(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_includes_via(args)?);
}

pub fn array_prototype_reduce(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_reduce_via(args)?);
}

pub fn array_prototype_push(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_push_via(args)?);
}

pub fn array_prototype_pop(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_pop_via()?);
}

pub fn array_prototype_shift(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_shift_via()?);
}

pub fn array_prototype_unshift(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_unshift_via(args)?);
}

pub fn array_prototype_reverse(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_reverse_via()?);
}

pub fn array_prototype_slice(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_slice_via(args)?);
}

pub fn array_prototype_splice(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_splice_via(args)?);
}

pub fn array_prototype_concat(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_concat_via(args)?);
}

pub fn array_prototype_join(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_join_via(args)?);
}

pub fn array_prototype_at(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_at_via(args)?);
}

pub fn array_prototype_fill(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_fill_via(args)?);
}

pub fn array_prototype_last_index_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_last_index_of_via(args)?);
}

pub fn array_prototype_reduce_right(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_reduce_right_via(args)?);
}

pub fn array_prototype_copy_within(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_copy_within_via(args)?);
}

pub fn array_prototype_flat(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_flat_via(args)?);
}

pub fn array_prototype_flat_map(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_flat_map_via(args)?);
}

pub fn array_prototype_to_reversed(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_to_reversed_via()?);
}

pub fn array_prototype_to_sorted(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_to_sorted_via(args)?);
}

pub fn array_prototype_to_spliced(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_to_spliced_via(args)?);
}

pub fn array_prototype_with(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_with_via(args)?);
}

pub fn array_prototype_to_locale_string(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_to_locale_string_via(args)?);
}

pub fn array_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_to_string_via()?);
}

pub fn array_prototype_sort(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_sort_via(args)?);
}

pub fn array_prototype_entries(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_entries_via()?);
}

pub fn array_prototype_keys(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_keys_via()?);
}

pub fn array_prototype_values(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_proto_values_via()?);
}

pub fn object_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_proto_to_string_via()?);
}

pub fn object_prototype_has_own_property(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_proto_has_own_property_via(args)?);
}

pub fn object_prototype_value_of(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_proto_value_of_via()?);
}

pub fn object_prototype_property_is_enumerable(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_proto_property_is_enumerable_via(args)?);
}

pub fn object_prototype_is_prototype_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_proto_is_prototype_of_via(args)?);
}

pub fn object_prototype_to_locale_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_proto_to_locale_string_via()?);
}

pub fn math_imul(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_imul_via(args)?);
}

pub fn math_fround(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_fround_via(args)?);
}

pub fn math_clz32(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_clz32_via(args)?);
}

pub fn array_is_array(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.array_is_array_via(args)?);
}

pub fn array_of(rt: &mut Runtime, this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.array_of_via(this, args)?);
}

pub fn number_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.number_proto_to_string_via(args)?);
}

pub fn number_prototype_to_locale_string(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.number_proto_to_locale_string_via(args)?);
}

pub fn string_from_char_code(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_from_char_code_via(args)?);
}

pub fn string_from_code_point(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_from_code_point_via(args)?);
}

pub fn error_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.error_proto_to_string_via()?);
}

pub fn symbol_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.symbol_proto_to_string_via()?);
}

pub fn bigint_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.bigint_proto_to_string_via(args)?);
}

pub fn function_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.function_proto_to_string_via()?);
}

pub fn date_prototype_get_time(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_time_via()?);
}

pub fn date_prototype_value_of(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_value_of_via()?);
}

pub fn date_prototype_to_iso_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_to_iso_string_via()?);
}

pub fn date_prototype_to_date_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_to_date_string_via()?);
}

pub fn date_prototype_to_time_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_to_time_string_via()?);
}

pub fn date_prototype_to_utc_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_to_utc_string_via()?);
}

pub fn date_prototype_to_string(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_to_string_via()?);
}

pub fn date_prototype_to_json(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_to_json_via()?);
}

pub fn date_prototype_get_full_year(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_full_year_via()?);
}

pub fn date_prototype_get_month(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_month_via()?);
}

pub fn date_prototype_get_date(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_date_via()?);
}

pub fn date_prototype_get_day(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_day_via()?);
}

pub fn date_prototype_get_hours(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_hours_via()?);
}

pub fn date_prototype_get_minutes(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_minutes_via()?);
}

pub fn date_prototype_get_seconds(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_seconds_via()?);
}

pub fn date_prototype_get_milliseconds(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_milliseconds_via()?);
}

pub fn date_prototype_set_time(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_time_via(args)?);
}

pub fn date_prototype_set_hours(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_hours_via(args)?);
}

pub fn date_prototype_set_minutes(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_minutes_via(args)?);
}

pub fn date_prototype_set_seconds(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_seconds_via(args)?);
}

pub fn date_prototype_set_milliseconds(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_milliseconds_via(args)?);
}

pub fn date_prototype_set_date(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_date_via(args)?);
}

pub fn date_prototype_set_month(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_month_via(args)?);
}

pub fn date_prototype_set_full_year(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_full_year_via(args)?);
}

pub fn string_raw(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.string_raw_via(args)?);
}

pub fn array_from(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.array_from_via(args)?);
}

pub fn date_now(rt: &mut Runtime, _this: Value, _args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.date_now_via()?);
}

pub fn date_parse(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.date_parse_via(args)?);
}

pub fn date_utc(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.date_utc_via(args)?);
}

pub fn math_random(rt: &mut Runtime, _this: Value, _args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_random_via()?);
}

pub fn date_prototype_get_timezone_offset(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_timezone_offset_via()?);
}

pub fn parse_int(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.parse_int_via(args)?);
}

pub fn parse_float(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.parse_float_via(args)?);
}

pub fn json_stringify(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.json_stringify_via(args)?);
}

pub fn json_parse(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.json_parse_via(args)?);
}

pub fn symbol_for(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.symbol_for_via(args)?);
}

pub fn symbol_key_for(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.symbol_key_for_via(args)?);
}

pub fn date_prototype_get_year(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_get_year_via()?);
}

pub fn date_prototype_set_year(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.date_proto_set_year_via(args)?);
}

pub fn object_group_by(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.object_group_by_via(args)?);
}

pub fn map_prototype_get(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_get_via(args)?);
}

pub fn map_prototype_set(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_set_via(args)?);
}

pub fn map_prototype_has(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_has_via(args)?);
}

pub fn map_prototype_delete(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_delete_via(args)?);
}

pub fn map_prototype_clear(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_clear_via()?);
}

pub fn map_prototype_for_each(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_for_each_via(args)?);
}

pub fn map_prototype_values(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_values_via()?);
}

pub fn map_prototype_keys(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_keys_via()?);
}

pub fn map_prototype_entries(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.map_proto_entries_via()?);
}

pub fn set_prototype_add(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_add_via(args)?);
}

pub fn set_prototype_has(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_has_via(args)?);
}

pub fn set_prototype_delete(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_delete_via(args)?);
}

pub fn set_prototype_clear(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_clear_via()?);
}

pub fn set_prototype_for_each(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_for_each_via(args)?);
}

pub fn set_prototype_union(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_union_via(args)?);
}

pub fn set_prototype_intersection(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_intersection_via(args)?);
}

pub fn set_prototype_difference(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_difference_via(args)?);
}

pub fn set_prototype_symmetric_difference(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_symmetric_difference_via(args)?);
}

pub fn set_prototype_is_subset_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_is_subset_of_via(args)?);
}

pub fn set_prototype_is_superset_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_is_superset_of_via(args)?);
}

pub fn set_prototype_is_disjoint_from(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.set_proto_is_disjoint_from_via(args)?);
}

pub fn object_keys(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let obj = rt.to_object(&target.clone())?;

    return Ok(rt.enumerable_own_keys(&obj.clone())?);
}

pub fn object_values(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let obj = rt.to_object(&target.clone())?;

    return Ok(rt.enumerable_own_values(&obj.clone())?);
}

pub fn object_entries(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let obj = rt.to_object(&target.clone())?;

    return Ok(rt.enumerable_own_entries(&obj.clone())?);
}

pub fn promise_resolve(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.promise_resolve_via(&x.clone())?);
}

pub fn promise_reject(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let reason = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.promise_reject_via(&reason.clone())?);
}

pub fn promise_prototype_then(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_then_via(args)?);
}

pub fn promise_prototype_catch(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_catch_via(args)?);
}

pub fn promise_prototype_finally(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_finally_via(args)?);
}

pub fn promise_all(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_all_via(args)?);
}

pub fn promise_all_settled(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_all_settled_via(args)?);
}

pub fn promise_any(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_any_via(args)?);
}

pub fn promise_race(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.promise_race_via(args)?);
}

pub fn promise_with_resolvers(
    rt: &mut Runtime,
    _this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    let p = rt.new_promise_value_via()?;

    let resolve_fn = {
        let p = p.clone();
        let __native = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
            let v = args.get(0).cloned().unwrap_or(Value::Undefined);
            let _ = &p;
            let _ = &v;

            rt.promise_settle_fulfilled_via(&p.clone(), &v.clone())?;
            Ok(Value::Undefined)
        });
        Value::Object(rt.alloc_object(__native))
    };

    let reject_fn = {
        let p = p.clone();
        let __native = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
            let v = args.get(0).cloned().unwrap_or(Value::Undefined);
            let _ = &p;
            let _ = &v;

            rt.promise_settle_rejected_via(&p.clone(), &v.clone())?;
            Ok(Value::Undefined)
        });
        Value::Object(rt.alloc_object(__native))
    };

    return Ok(rt.promise_with_resolvers_assemble_via(
        &p.clone(),
        &resolve_fn.clone(),
        &reject_fn.clone(),
    )?);
}

pub fn promise_all_resolve_element_factory(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let index = args.get(0).cloned().unwrap_or(Value::Undefined);

    let values = args.get(1).cloned().unwrap_or(Value::Undefined);

    let already = args.get(2).cloned().unwrap_or(Value::Undefined);

    let remaining = args.get(3).cloned().unwrap_or(Value::Undefined);

    let cap_resolve = args.get(4).cloned().unwrap_or(Value::Undefined);

    return Ok({
        let index = index.clone();
        let values = values.clone();
        let already = already.clone();
        let remaining = remaining.clone();
        let cap_resolve = cap_resolve.clone();
        let __root_index = index.clone();
        let __root_values = values.clone();
        let __root_already = already.clone();
        let __root_remaining = remaining.clone();
        let __root_cap_resolve = cap_resolve.clone();
        let __native = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
            let x = args.get(0).cloned().unwrap_or(Value::Undefined);
            let _ = &index;
            let _ = &values;
            let _ = &already;
            let _ = &remaining;
            let _ = &cap_resolve;
            let _ = &x;

            if !crate::abstract_ops::to_boolean(&rt.cell_check_and_set_via(&already.clone())?) {

                return Ok(Value::Undefined);
            }

            rt.cell_array_set_via(&values.clone(), &index.clone(), &x.clone())?;

            rt.promise_all_maybe_complete_via(
                &values.clone(),
                &remaining.clone(),
                &cap_resolve.clone(),
            )?;
            Ok(Value::Undefined)
        });
        let __id = rt.alloc_object(__native);
        {
            let __obj = rt.obj_mut(__id);
            __obj.set_own_internal("__promise_element_index".into(), __root_index);
            __obj.set_own_internal("__promise_element_values".into(), __root_values);
            __obj.set_own_internal("__promise_element_already".into(), __root_already);
            __obj.set_own_internal("__promise_element_remaining".into(), __root_remaining);
            __obj.set_own_internal("__promise_element_cap_resolve".into(), __root_cap_resolve);
        }
        Value::Object(__id)
    });
}

pub fn promise_all_settled_resolve_element_factory(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let index = args.get(0).cloned().unwrap_or(Value::Undefined);

    let values = args.get(1).cloned().unwrap_or(Value::Undefined);

    let already = args.get(2).cloned().unwrap_or(Value::Undefined);

    let remaining = args.get(3).cloned().unwrap_or(Value::Undefined);

    let cap_resolve = args.get(4).cloned().unwrap_or(Value::Undefined);

    return Ok({
        let index = index.clone();
        let values = values.clone();
        let already = already.clone();
        let remaining = remaining.clone();
        let cap_resolve = cap_resolve.clone();
        let __root_index = index.clone();
        let __root_values = values.clone();
        let __root_already = already.clone();
        let __root_remaining = remaining.clone();
        let __root_cap_resolve = cap_resolve.clone();
        let __native = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
            let x = args.get(0).cloned().unwrap_or(Value::Undefined);
            let _ = &index;
            let _ = &values;
            let _ = &already;
            let _ = &remaining;
            let _ = &cap_resolve;
            let _ = &x;

            if !crate::abstract_ops::to_boolean(&rt.cell_check_and_set_via(&already.clone())?) {

                return Ok(Value::Undefined);
            }

            let entry = rt.make_settled_fulfilled_entry_via(&x.clone())?;

            rt.cell_array_set_via(&values.clone(), &index.clone(), &entry.clone())?;

            rt.promise_all_maybe_complete_via(
                &values.clone(),
                &remaining.clone(),
                &cap_resolve.clone(),
            )?;
            Ok(Value::Undefined)
        });
        let __id = rt.alloc_object(__native);
        {
            let __obj = rt.obj_mut(__id);
            __obj.set_own_internal("__promise_element_index".into(), __root_index);
            __obj.set_own_internal("__promise_element_values".into(), __root_values);
            __obj.set_own_internal("__promise_element_already".into(), __root_already);
            __obj.set_own_internal("__promise_element_remaining".into(), __root_remaining);
            __obj.set_own_internal("__promise_element_cap_resolve".into(), __root_cap_resolve);
        }
        Value::Object(__id)
    });
}

pub fn promise_all_settled_reject_element_factory(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let index = args.get(0).cloned().unwrap_or(Value::Undefined);

    let values = args.get(1).cloned().unwrap_or(Value::Undefined);

    let already = args.get(2).cloned().unwrap_or(Value::Undefined);

    let remaining = args.get(3).cloned().unwrap_or(Value::Undefined);

    let cap_resolve = args.get(4).cloned().unwrap_or(Value::Undefined);

    return Ok({
        let index = index.clone();
        let values = values.clone();
        let already = already.clone();
        let remaining = remaining.clone();
        let cap_resolve = cap_resolve.clone();
        let __root_index = index.clone();
        let __root_values = values.clone();
        let __root_already = already.clone();
        let __root_remaining = remaining.clone();
        let __root_cap_resolve = cap_resolve.clone();
        let __native = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
            let x = args.get(0).cloned().unwrap_or(Value::Undefined);
            let _ = &index;
            let _ = &values;
            let _ = &already;
            let _ = &remaining;
            let _ = &cap_resolve;
            let _ = &x;

            if !crate::abstract_ops::to_boolean(&rt.cell_check_and_set_via(&already.clone())?) {

                return Ok(Value::Undefined);
            }

            let entry = rt.make_settled_rejected_entry_via(&x.clone())?;

            rt.cell_array_set_via(&values.clone(), &index.clone(), &entry.clone())?;

            rt.promise_all_maybe_complete_via(
                &values.clone(),
                &remaining.clone(),
                &cap_resolve.clone(),
            )?;
            Ok(Value::Undefined)
        });
        let __id = rt.alloc_object(__native);
        {
            let __obj = rt.obj_mut(__id);
            __obj.set_own_internal("__promise_element_index".into(), __root_index);
            __obj.set_own_internal("__promise_element_values".into(), __root_values);
            __obj.set_own_internal("__promise_element_already".into(), __root_already);
            __obj.set_own_internal("__promise_element_remaining".into(), __root_remaining);
            __obj.set_own_internal("__promise_element_cap_resolve".into(), __root_cap_resolve);
        }
        Value::Object(__id)
    });
}

pub fn promise_any_reject_element_factory(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let index = args.get(0).cloned().unwrap_or(Value::Undefined);

    let errors = args.get(1).cloned().unwrap_or(Value::Undefined);

    let already = args.get(2).cloned().unwrap_or(Value::Undefined);

    let remaining = args.get(3).cloned().unwrap_or(Value::Undefined);

    let cap_reject = args.get(4).cloned().unwrap_or(Value::Undefined);

    return Ok({
        let index = index.clone();
        let errors = errors.clone();
        let already = already.clone();
        let remaining = remaining.clone();
        let cap_reject = cap_reject.clone();
        let __root_index = index.clone();
        let __root_errors = errors.clone();
        let __root_already = already.clone();
        let __root_remaining = remaining.clone();
        let __root_cap_reject = cap_reject.clone();
        let __native = crate::intrinsics::make_native_non_ctor("", 1, move |rt, args| {
            let x = args.get(0).cloned().unwrap_or(Value::Undefined);
            let _ = &index;
            let _ = &errors;
            let _ = &already;
            let _ = &remaining;
            let _ = &cap_reject;
            let _ = &x;

            if !crate::abstract_ops::to_boolean(&rt.cell_check_and_set_via(&already.clone())?) {

                return Ok(Value::Undefined);
            }

            rt.cell_array_set_via(&errors.clone(), &index.clone(), &x.clone())?;

            rt.promise_any_maybe_reject_via(
                &errors.clone(),
                &remaining.clone(),
                &cap_reject.clone(),
            )?;
            Ok(Value::Undefined)
        });
        let __id = rt.alloc_object(__native);
        {
            let __obj = rt.obj_mut(__id);
            __obj.set_own_internal("__promise_element_index".into(), __root_index);
            __obj.set_own_internal("__promise_element_errors".into(), __root_errors);
            __obj.set_own_internal("__promise_element_already".into(), __root_already);
            __obj.set_own_internal("__promise_element_remaining".into(), __root_remaining);
            __obj.set_own_internal("__promise_element_cap_reject".into(), __root_cap_reject);
        }
        Value::Object(__id)
    });
}

pub fn object_define_property(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    let desc = args.get(2).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_define_property_via(&target.clone(), &key.clone(), &desc.clone())?);
}

pub fn object_define_properties(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let props = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_define_properties_via(&target.clone(), &props.clone())?);
}

pub fn object_get_own_property_descriptor(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_get_own_property_descriptor_via(&obj.clone(), &key.clone())?);
}

pub fn object_get_own_property_descriptors(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let obj = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_get_own_property_descriptors_via(&obj.clone())?);
}

pub fn object_create(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let proto = args.get(0).cloned().unwrap_or(Value::Undefined);

    let props = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_create_via(&proto.clone(), &props.clone())?);
}

pub fn object_proto_define_getter(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let this_ = this.clone();

    let key = args.get(0).cloned().unwrap_or(Value::Undefined);

    let fn_ = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_proto_define_getter_via(&this_.clone(), &key.clone(), &fn_.clone())?);
}

pub fn object_proto_define_setter(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let this_ = this.clone();

    let key = args.get(0).cloned().unwrap_or(Value::Undefined);

    let fn_ = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_proto_define_setter_via(&this_.clone(), &key.clone(), &fn_.clone())?);
}

pub fn object_proto_lookup_getter(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let this_ = this.clone();

    let key = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_proto_lookup_getter_via(&this_.clone(), &key.clone())?);
}

pub fn object_proto_lookup_setter(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let this_ = this.clone();

    let key = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_proto_lookup_setter_via(&this_.clone(), &key.clone())?);
}

pub fn array_set_length(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let desc = args.get(1).cloned().unwrap_or(Value::Undefined);

    if rt.has_property_via(&desc.clone(), "configurable") {

        if crate::abstract_ops::to_boolean(&rt.spec_get(&desc.clone(), "configurable")?) {

            return Err(RuntimeError::TypeError(
                "Array length: configurable is false".into(),
            ));
        }
    }

    if rt.has_property_via(&desc.clone(), "enumerable") {

        if crate::abstract_ops::to_boolean(&rt.spec_get(&desc.clone(), "enumerable")?) {

            return Err(RuntimeError::TypeError(
                "Array length: enumerable is false".into(),
            ));
        }
    }

    if rt.has_property_via(&desc.clone(), "get") {

        return Err(RuntimeError::TypeError(
            "Array length cannot be accessor (get)".into(),
        ));
    }

    if rt.has_property_via(&desc.clone(), "set") {

        return Err(RuntimeError::TypeError(
            "Array length cannot be accessor (set)".into(),
        ));
    }

    let cur_writable = rt.array_length_writable_via(&target.clone())?;

    let old_len = rt.array_length_value_via(&target.clone())?;

    if !rt.has_property_via(&desc.clone(), "value") {

        if rt.has_property_via(&desc.clone(), "writable") {

            let new_w = rt.spec_get(&desc.clone(), "writable")?;

            if !crate::abstract_ops::to_boolean(&cur_writable.clone()) {

                if crate::abstract_ops::to_boolean(&new_w.clone()) {

                    return Err(RuntimeError::TypeError(
                        "Cannot promote Array length to writable".into(),
                    ));
                }
            }

            rt.array_length_set_internal_via(&target.clone(), &old_len.clone(), &new_w.clone())?;
        }

        return Ok(target.clone());
    }

    let raw_value = rt.spec_get(&desc.clone(), "value")?;

    let new_len = rt.to_uint32_strict_via(&raw_value.clone())?;

    if !crate::abstract_ops::to_boolean(&cur_writable.clone()) {

        if !crate::abstract_ops::is_strictly_equal(&new_len.clone(), &old_len.clone()) {

            return Err(RuntimeError::TypeError(
                "Cannot change non-writable Array length".into(),
            ));
        }
    }

    let mut new_writable = cur_writable.clone();

    if rt.has_property_via(&desc.clone(), "writable") {

        new_writable = rt.spec_get(&desc.clone(), "writable")?;
    }

    if rt.coerce_to_number(&new_len.clone())? < rt.coerce_to_number(&old_len.clone())? {

        let mut idx = Value::Number(
            rt.coerce_to_number(&old_len.clone())? - rt.coerce_to_number(&Value::Number(1_f64))?,
        );

        while rt.coerce_to_number(&idx.clone())? >= rt.coerce_to_number(&new_len.clone())? {

            let idx_key = rt.number_to_string_key_via(&idx.clone())?;

            let deleted = rt.delete_own_via(&target.clone(), &idx_key.clone())?;

            if !crate::abstract_ops::to_boolean(&deleted.clone()) {

                let stuck_len = Value::Number(
                    rt.coerce_to_number(&idx.clone())?
                        + rt.coerce_to_number(&Value::Number(1_f64))?,
                );

                rt.array_length_set_internal_via(
                    &target.clone(),
                    &stuck_len.clone(),
                    &new_writable.clone(),
                )?;

                return Err(RuntimeError::TypeError(
                    "Cannot truncate Array: non-configurable element".into(),
                ));
            }

            idx = Value::Number(
                rt.coerce_to_number(&idx.clone())? - rt.coerce_to_number(&Value::Number(1_f64))?,
            );
        }
    }

    rt.array_length_set_internal_via(&target.clone(), &new_len.clone(), &new_writable.clone())?;

    return Ok(target.clone());
}

pub fn json_serialize_property(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let holder = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    let mut value = rt.json_get_property_via(&holder.clone(), &key.clone())?;

    value = rt.json_apply_to_json_via(&value.clone(), &key.clone())?;

    value = rt.json_apply_replacer_via(&holder.clone(), &value.clone(), &key.clone())?;

    value = rt.json_unwrap_wrapper_via(&value.clone())?;

    if crate::abstract_ops::is_strictly_equal(&value.clone(), &Value::Null) {

        return Ok(Value::String(std::rc::Rc::new(
            crate::value::JsString::from("null".to_string()),
        )));
    }

    if crate::abstract_ops::is_strictly_equal(&value.clone(), &Value::Boolean(true)) {

        return Ok(Value::String(std::rc::Rc::new(
            crate::value::JsString::from("true".to_string()),
        )));
    }

    if crate::abstract_ops::is_strictly_equal(&value.clone(), &Value::Boolean(false)) {

        return Ok(Value::String(std::rc::Rc::new(
            crate::value::JsString::from("false".to_string()),
        )));
    }

    if crate::abstract_ops::is_strictly_equal(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            rt.type_of_value(&value.clone()).to_string(),
        ))),
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "string".to_string(),
        ))),
    ) {

        return Ok(rt.json_quote_string_via(&value.clone())?);
    }

    if crate::abstract_ops::is_strictly_equal(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            rt.type_of_value(&value.clone()).to_string(),
        ))),
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "number".to_string(),
        ))),
    ) {

        return Ok(rt.json_format_number_via(&value.clone())?);
    }

    if crate::abstract_ops::is_strictly_equal(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            rt.type_of_value(&value.clone()).to_string(),
        ))),
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "bigint".to_string(),
        ))),
    ) {

        return Err(RuntimeError::TypeError(
            "Do not know how to serialize a BigInt".into(),
        ));
    }

    if crate::abstract_ops::is_strictly_equal(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            rt.type_of_value(&value.clone()).to_string(),
        ))),
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "object".to_string(),
        ))),
    ) {

        return Ok(rt.json_serialize_compound_via(&value.clone())?);
    }

    return Ok(Value::Undefined);
}

pub fn object_assign_source_into(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let source = args.get(1).cloned().unwrap_or(Value::Undefined);

    if crate::abstract_ops::is_strictly_equal(&source.clone(), &Value::Null) {

        return Ok(target.clone());
    }

    if crate::abstract_ops::is_strictly_equal(&source.clone(), &Value::Undefined) {

        return Ok(target.clone());
    }

    let from = rt.to_object_strict_via(&source.clone())?;

    let keys = rt.reflect_own_keys_via(&from.clone())?;

    let len: usize = rt.length_of_array_like(&keys.clone())?;

    let mut k: usize = 0_usize;

    while k.clone() < len.clone() {

        let key = {
            let _temporary_value_roots =
                rt.push_temporary_value_roots(&[target.clone(), from.clone(), keys.clone()]);
            rt.spec_get(&keys.clone(), &k.clone().to_string())?
        };

        let keep = {
            let _temporary_value_roots = rt.push_temporary_value_roots(&[
                target.clone(),
                from.clone(),
                keys.clone(),
                key.clone(),
            ]);
            rt.is_enumerable_own_via(&from.clone(), &key.clone())?
        };

        if crate::abstract_ops::is_strictly_equal(&keep.clone(), &Value::Boolean(true)) {

            let propValue = {
                let _temporary_value_roots = rt.push_temporary_value_roots(&[
                    target.clone(),
                    from.clone(),
                    keys.clone(),
                    key.clone(),
                ]);
                rt.get_via(&from.clone(), &key.clone())?
            };

            {
                let _temporary_value_roots = rt.push_temporary_value_roots(&[
                    target.clone(),
                    from.clone(),
                    keys.clone(),
                    key.clone(),
                    propValue.clone(),
                ]);
                rt.set_or_throw_via(&target.clone(), &key.clone(), &propValue.clone())?;
                Value::Undefined
            };
        }

        k = k.clone() + 1_usize;
    }

    return Ok(target.clone());
}

pub fn to_primitive(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let value = args.get(0).cloned().unwrap_or(Value::Undefined);

    let hint = args.get(1).cloned().unwrap_or(Value::Undefined);

    if !matches!(value.clone(), Value::Object(_)) {

        return Ok(value.clone());
    }

    let exotic = rt.get_method(&value.clone(), "@@toPrimitive")?;

    if rt.is_callable(&exotic.clone()) {

        let result = rt.call_function(
            exotic.clone().clone(),
            value.clone().clone(),
            vec![hint.clone()],
        )?;

        if !matches!(result.clone(), Value::Object(_)) {

            return Ok(result.clone());
        }

        return Err(RuntimeError::TypeError(
            "@@toPrimitive returned an object".into(),
        ));
    }

    let mut method1 = Value::String(std::rc::Rc::new(crate::value::JsString::from(
        "valueOf".to_string(),
    )));

    let mut method2 = Value::String(std::rc::Rc::new(crate::value::JsString::from(
        "toString".to_string(),
    )));

    if crate::abstract_ops::is_strictly_equal(
        &hint.clone(),
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "string".to_string(),
        ))),
    ) {

        method1 = Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "toString".to_string(),
        )));

        method2 = Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "valueOf".to_string(),
        )));
    }

    let m1 = rt.get_via(&value.clone(), &method1.clone())?;

    if rt.is_callable(&m1.clone()) {

        let r1 = rt.call_function(m1.clone().clone(), value.clone().clone(), vec![])?;

        if !matches!(r1.clone(), Value::Object(_)) {

            return Ok(r1.clone());
        }
    }

    let m2 = rt.get_via(&value.clone(), &method2.clone())?;

    if rt.is_callable(&m2.clone()) {

        let r2 = rt.call_function(m2.clone().clone(), value.clone().clone(), vec![])?;

        if !matches!(r2.clone(), Value::Object(_)) {

            return Ok(r2.clone());
        }
    }

    return Err(RuntimeError::TypeError(
        "Cannot convert object to primitive value".into(),
    ));
}

pub fn object_get_prototype_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let obj = rt.to_object(&target.clone())?;

    return Ok(rt.get_prototype_of_via(&obj.clone())?);
}

pub fn object_set_prototype_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);

    let o = rt.to_object(&target.clone())?;

    return Ok(rt.set_prototype_of_via(&o.clone(), &proto.clone())?);
}

pub fn object_is_extensible(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.is_extensible_via(&target.clone())?);
}

pub fn object_is_frozen(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.is_frozen_via(&target.clone())?);
}

pub fn object_is_sealed(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.is_sealed_via(&target.clone())?);
}

pub fn object_freeze(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_freeze_via(&target.clone())?);
}

pub fn object_seal(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_seal_via(&target.clone())?);
}

pub fn object_prevent_extensions(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_prevent_extensions_via(&target.clone())?);
}

pub fn object_has_own(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_has_own_via(&target.clone(), &key.clone())?);
}

pub fn object_is(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let value1 = args.get(0).cloned().unwrap_or(Value::Undefined);

    let value2 = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_is_via(&value1.clone(), &value2.clone())?);
}

pub fn number_is_finite(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let number = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_is_finite_via(&number.clone())?);
}

pub fn number_is_integer(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let number = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_is_integer_via(&number.clone())?);
}

pub fn number_is_nan(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let number = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_is_nan_via(&number.clone())?);
}

pub fn number_is_safe_integer(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let number = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_is_safe_integer_via(&number.clone())?);
}

pub fn global_is_nan(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let number = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.global_is_nan_via(&number.clone())?);
}

pub fn global_is_finite(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let number = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.global_is_finite_via(&number.clone())?);
}

pub fn math_abs(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "abs".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_floor(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "floor".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_ceil(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "ceil".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_round(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "round".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_trunc(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "trunc".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_sqrt(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "sqrt".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_cbrt(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "cbrt".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_sign(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "sign".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_exp(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "exp".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_expm1(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "expm1".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_log(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "log".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_log1p(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "log1p".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_log2(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "log2".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_log10(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "log10".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_sin(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "sin".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_cos(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "cos".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_tan(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "tan".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_asin(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "asin".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_acos(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "acos".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_atan(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "atan".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_sinh(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "sinh".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_cosh(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "cosh".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_tanh(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "tanh".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_asinh(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "asinh".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_acosh(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "acosh".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn math_atanh(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_unary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "atanh".to_string(),
        ))),
        &x.clone(),
    )?);
}

pub fn reflect_has(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_has_via(&target.clone(), &key.clone())?);
}

pub fn reflect_get(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    let receiver = args.get(2).cloned().unwrap_or(target.clone());

    return Ok(rt.reflect_get_via_receiver(&target.clone(), &key.clone(), &receiver.clone())?);
}

pub fn reflect_set(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    let value = args.get(2).cloned().unwrap_or(Value::Undefined);

    let receiver = args.get(3).cloned().unwrap_or(target.clone());

    return Ok(rt.reflect_set_via_receiver(
        &target.clone(),
        &key.clone(),
        &value.clone(),
        &receiver.clone(),
    )?);
}

pub fn reflect_delete_property(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let key = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_delete_property_via(&target.clone(), &key.clone())?);
}

pub fn reflect_own_keys(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_own_keys_via(&target.clone())?);
}

pub fn reflect_get_prototype_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_get_prototype_of_via(&target.clone())?);
}

pub fn reflect_set_prototype_of(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let proto = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_set_prototype_of_via(&target.clone(), &proto.clone())?);
}

pub fn reflect_is_extensible(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_is_extensible_via(&target.clone())?);
}

pub fn reflect_prevent_extensions(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.reflect_prevent_extensions_via(&target.clone())?);
}

pub fn math_pow(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    let y = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_binary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "pow".to_string(),
        ))),
        &x.clone(),
        &y.clone(),
    )?);
}

pub fn math_atan2(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    let x = args.get(0).cloned().unwrap_or(Value::Undefined);

    let y = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.math_binary_op_via(
        &Value::String(std::rc::Rc::new(crate::value::JsString::from(
            "atan2".to_string(),
        ))),
        &x.clone(),
        &y.clone(),
    )?);
}

pub fn math_max(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_max_via(args)?);
}

pub fn math_min(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_min_via(args)?);
}

pub fn math_hypot(rt: &mut Runtime, _this: Value, args: &[Value]) -> Result<Value, RuntimeError> {

    return Ok(rt.math_hypot_via(args)?);
}

pub fn object_get_own_property_names(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let obj = rt.to_object(&target.clone())?;

    return Ok(rt.own_property_names_via(&obj.clone())?);
}

pub fn object_get_own_property_symbols(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let obj = rt.to_object(&target.clone())?;

    return Ok(rt.own_property_symbols_via(&obj.clone())?);
}

pub fn object_assign(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_assign_via(
        &target.clone(),
        if args.len() > 1 { &args[1..] } else { &[] },
    )?);
}

pub fn object_from_entries(
    rt: &mut Runtime,
    _this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let iter = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.object_from_entries_via(&iter.clone())?);
}

pub fn number_prototype_to_fixed(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let digits = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_proto_to_fixed_via(&this.clone(), &digits.clone())?);
}

pub fn number_prototype_value_of(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.number_proto_value_of_via(&this.clone())?);
}

pub fn number_prototype_to_exponential(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let digits = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_proto_to_exponential_via(&this.clone(), &digits.clone())?);
}

pub fn number_prototype_to_precision(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let precision = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.number_proto_to_precision_via(&this.clone(), &precision.clone())?);
}

pub fn boolean_prototype_value_of(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.boolean_proto_value_of_via(&this.clone())?);
}

pub fn boolean_prototype_to_string(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.boolean_proto_to_string_via(&this.clone())?);
}

pub fn string_prototype_char_at(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let pos = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_char_at_via(&this.clone(), &pos.clone())?);
}

pub fn string_prototype_char_code_at(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let pos = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_char_code_at_via(&this.clone(), &pos.clone())?);
}

pub fn string_prototype_concat(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_concat_via(&this.clone(), args)?);
}

pub fn string_prototype_to_lower_case(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_to_lower_case_via(&this.clone())?);
}

pub fn string_prototype_to_upper_case(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_to_upper_case_via(&this.clone())?);
}

pub fn string_prototype_to_locale_lower_case(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let locales = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_to_locale_lower_case_via(&this.clone(), &locales.clone())?);
}

pub fn string_prototype_to_locale_upper_case(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let locales = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_to_locale_upper_case_via(&this.clone(), &locales.clone())?);
}

pub fn string_prototype_trim(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_trim_via(&this.clone())?);
}

pub fn string_prototype_trim_start(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_trim_start_via(&this.clone())?);
}

pub fn string_prototype_trim_end(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_trim_end_via(&this.clone())?);
}

pub fn string_prototype_trim_left(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_trim_start_via(&this.clone())?);
}

pub fn string_prototype_trim_right(
    rt: &mut Runtime,
    this: Value,
    _args: &[Value],
) -> Result<Value, RuntimeError> {

    return Ok(rt.string_proto_trim_end_via(&this.clone())?);
}

pub fn string_prototype_repeat(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let count = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_repeat_via(&this.clone(), &count.clone())?);
}

pub fn string_prototype_pad_start(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let pad = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_pad_start_via(&this.clone(), &target.clone(), &pad.clone())?);
}

pub fn string_prototype_pad_end(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let target = args.get(0).cloned().unwrap_or(Value::Undefined);

    let pad = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_pad_end_via(&this.clone(), &target.clone(), &pad.clone())?);
}

pub fn string_prototype_slice(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let start = args.get(0).cloned().unwrap_or(Value::Undefined);

    let end = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_slice_via(&this.clone(), &start.clone(), &end.clone())?);
}

pub fn string_prototype_substring(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let start = args.get(0).cloned().unwrap_or(Value::Undefined);

    let end = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_substring_via(&this.clone(), &start.clone(), &end.clone())?);
}

pub fn string_prototype_substr(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let start = args.get(0).cloned().unwrap_or(Value::Undefined);

    let length = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_substr_via(&this.clone(), &start.clone(), &length.clone())?);
}

pub fn string_prototype_index_of(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let position = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_index_of_via(&this.clone(), &search.clone(), &position.clone())?);
}

pub fn string_prototype_last_index_of(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let position = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_last_index_of_via(
        &this.clone(),
        &search.clone(),
        &position.clone(),
    )?);
}

pub fn string_prototype_includes(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let position = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_includes_via(&this.clone(), &search.clone(), &position.clone())?);
}

pub fn string_prototype_starts_with(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let position = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_starts_with_via(
        &this.clone(),
        &search.clone(),
        &position.clone(),
    )?);
}

pub fn string_prototype_ends_with(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let position = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_ends_with_via(&this.clone(), &search.clone(), &position.clone())?);
}

pub fn string_prototype_code_point_at(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let pos = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_code_point_at_via(&this.clone(), &pos.clone())?);
}

pub fn string_prototype_at(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let index = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_at_via(&this.clone(), &index.clone())?);
}

pub fn string_prototype_normalize(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let form = args.get(0).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_normalize_via(&this.clone(), &form.clone())?);
}

pub fn string_prototype_locale_compare(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let that = args.get(0).cloned().unwrap_or(Value::Undefined);

    let locales = args.get(1).cloned().unwrap_or(Value::Undefined);

    let options = args.get(2).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_locale_compare_intl_via(
        &this.clone(),
        &that.clone(),
        &locales.clone(),
        &options.clone(),
    )?);
}

pub fn string_prototype_split(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let separator = args.get(0).cloned().unwrap_or(Value::Undefined);

    let limit = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_split_via(&this.clone(), &separator.clone(), &limit.clone())?);
}

pub fn string_prototype_replace(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let replacement = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_replace_via(
        &this.clone(),
        &search.clone(),
        &replacement.clone(),
    )?);
}

pub fn string_prototype_replace_all(
    rt: &mut Runtime,
    this: Value,
    args: &[Value],
) -> Result<Value, RuntimeError> {

    let search = args.get(0).cloned().unwrap_or(Value::Undefined);

    let replacement = args.get(1).cloned().unwrap_or(Value::Undefined);

    return Ok(rt.string_proto_replace_all_via(
        &this.clone(),
        &search.clone(),
        &replacement.clone(),
    )?);
}
