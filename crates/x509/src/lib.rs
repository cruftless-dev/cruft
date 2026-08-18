
use rusty_asn1_der::*;

#[derive(Debug, Clone)]
pub enum X509Error {
    DerParse(DerError),
    UnsupportedVersion(i64),
    UnsupportedSigAlg(String),
    UnsupportedPubKeyAlg(String),
    InvalidSpki,
    InvalidValidity,
    InvalidSignature,
    InvalidExtension(String),
    UnknownCriticalExtension(String),
    CryptoFail(String),
    PemBadHeader,
    PemBadBase64,
}

impl From<DerError> for X509Error {
    fn from(e: DerError) -> Self {
        X509Error::DerParse(e)
    }
}

impl std::fmt::Display for X509Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            X509Error::DerParse(e) => write!(f, "DER parse: {}", e),
            X509Error::UnsupportedVersion(v) => write!(f, "unsupported X.509 version {}", v),
            X509Error::UnsupportedSigAlg(o) => write!(f, "unsupported signature algorithm {}", o),
            X509Error::UnsupportedPubKeyAlg(o) => {
                write!(f, "unsupported public key algorithm {}", o)
            }
            X509Error::InvalidSpki => write!(f, "invalid SubjectPublicKeyInfo"),
            X509Error::InvalidValidity => write!(f, "invalid validity period"),
            X509Error::InvalidSignature => write!(f, "signature verification failed"),
            X509Error::InvalidExtension(s) => write!(f, "invalid extension: {}", s),
            X509Error::UnknownCriticalExtension(oid) => {
                write!(f, "unknown critical X.509 extension {}", oid)
            }
            X509Error::CryptoFail(s) => write!(f, "crypto: {}", s),
            X509Error::PemBadHeader => write!(f, "PEM bad header (expected BEGIN/END CERTIFICATE)"),
            X509Error::PemBadBase64 => write!(f, "PEM base64 decode failed"),
        }
    }
}

impl std::error::Error for X509Error {}

pub const OID_RSA_ENCRYPTION: &str = "1.2.840.113549.1.1.1";
pub const OID_SHA1_WITH_RSA: &str = "1.2.840.113549.1.1.5";
pub const OID_SHA256_WITH_RSA: &str = "1.2.840.113549.1.1.11";
pub const OID_SHA384_WITH_RSA: &str = "1.2.840.113549.1.1.12";
pub const OID_SHA512_WITH_RSA: &str = "1.2.840.113549.1.1.13";
pub const OID_EC_PUBLIC_KEY: &str = "1.2.840.10045.2.1";
pub const OID_ECDSA_WITH_SHA256: &str = "1.2.840.10045.4.3.2";
pub const OID_ECDSA_WITH_SHA384: &str = "1.2.840.10045.4.3.3";
pub const OID_ECDSA_WITH_SHA512: &str = "1.2.840.10045.4.3.4";
pub const OID_P256_CURVE: &str = "1.2.840.10045.3.1.7";
pub const OID_P384_CURVE: &str = "1.3.132.0.34";
pub const OID_P521_CURVE: &str = "1.3.132.0.35";

pub const OID_RDN_CN: &str = "2.5.4.3";
pub const OID_RDN_C: &str = "2.5.4.6";
pub const OID_RDN_O: &str = "2.5.4.10";
pub const OID_RDN_OU: &str = "2.5.4.11";
pub const OID_SUBJECT_ALT_NAME: &str = "2.5.29.17";
pub const OID_BASIC_CONSTRAINTS: &str = "2.5.29.19";
pub const OID_KEY_USAGE: &str = "2.5.29.15";
pub const OID_EXTENDED_KEY_USAGE: &str = "2.5.29.37";
pub const OID_NAME_CONSTRAINTS: &str = "2.5.29.30";

const MAX_X509_DER_NESTING_DEPTH: usize = 64;

#[derive(Debug, Clone)]
pub struct AlgorithmIdentifier {
    pub oid: String,

    pub params: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct DistinguishedName {

    pub attributes: Vec<(String, String)>,

    pub raw_der: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Validity {

    pub not_before: Vec<u8>,
    pub not_before_tag: u8,
    pub not_after: Vec<u8>,
    pub not_after_tag: u8,
}

#[derive(Debug, Clone)]
pub enum PublicKey {

    Rsa { n: Vec<u8>, e: Vec<u8> },

    Ec { curve_oid: String, point: Vec<u8> },
}

#[derive(Debug, Clone)]
pub struct SubjectPublicKeyInfo {
    pub algorithm: AlgorithmIdentifier,
    pub key: PublicKey,

    pub raw_der: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct Extension {
    pub oid: String,
    pub critical: bool,
    pub value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BasicConstraints {
    pub ca: bool,
    pub path_len_constraint: Option<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KeyUsage {
    pub digital_signature: bool,
    pub non_repudiation: bool,
    pub key_encipherment: bool,
    pub data_encipherment: bool,
    pub key_agreement: bool,
    pub key_cert_sign: bool,
    pub crl_sign: bool,
    pub encipher_only: bool,
    pub decipher_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtendedKeyUsage {
    pub usages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneralSubtree {
    pub base: GeneralName,
    pub minimum: u64,
    pub maximum: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameConstraints {
    pub permitted_subtrees: Vec<GeneralSubtree>,
    pub excluded_subtrees: Vec<GeneralSubtree>,
}

#[derive(Debug, Clone)]
pub struct Certificate {
    pub version: u8,
    pub serial_number: Vec<u8>,
    pub signature_algorithm: AlgorithmIdentifier,
    pub issuer: DistinguishedName,
    pub validity: Validity,
    pub subject: DistinguishedName,
    pub subject_public_key_info: SubjectPublicKeyInfo,
    pub extensions: Vec<Extension>,

    pub tbs_certificate: Vec<u8>,

    pub signature_value: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeneralName {
    DnsName(String),
    IpAddress(Vec<u8>),
    Other { tag: u8, value: Vec<u8> },
}

impl Certificate {
    pub fn common_name(&self) -> Option<&str> {
        self.subject
            .attributes
            .iter()
            .find(|(oid, _)| oid == OID_RDN_CN)
            .map(|(_, value)| value.as_str())
    }

    pub fn subject_alt_names(&self) -> Result<Vec<GeneralName>, X509Error> {
        let mut names = Vec::new();
        for ext in &self.extensions {
            if ext.oid != OID_SUBJECT_ALT_NAME {
                continue;
            }
            let seq = parse_single_bounded(&ext.value)?;
            if seq.tag != TAG_SEQUENCE {
                return Err(X509Error::DerParse(DerError::WrongTag {
                    expected: TAG_SEQUENCE,
                    actual: seq.tag,
                }));
            }
            let mut r = DerReader::new(seq.content);
            while !r.is_empty() {
                let name = r.read_tlv()?;
                match name.tag {
                    0x82 => names.push(GeneralName::DnsName(decode_dns_name(name.content)?)),
                    0x87 => names.push(GeneralName::IpAddress(decode_ip_address(name.content)?)),

                    0x81 | 0x86 => names.push(GeneralName::Other {
                        tag: name.tag,
                        value: name.content.to_vec(),
                    }),
                    _ => {}
                }
            }
        }
        Ok(names)
    }

    pub fn basic_constraints(&self) -> Result<Option<BasicConstraints>, X509Error> {
        self.parse_first_extension(OID_BASIC_CONSTRAINTS, parse_basic_constraints)
    }

    pub fn key_usage(&self) -> Result<Option<KeyUsage>, X509Error> {
        self.parse_first_extension(OID_KEY_USAGE, parse_key_usage)
    }

    pub fn extended_key_usage(&self) -> Result<Option<ExtendedKeyUsage>, X509Error> {
        self.parse_first_extension(OID_EXTENDED_KEY_USAGE, parse_extended_key_usage)
    }

    pub fn name_constraints(&self) -> Result<Option<NameConstraints>, X509Error> {
        self.parse_first_extension(OID_NAME_CONSTRAINTS, parse_name_constraints)
    }

    pub fn is_ca(&self) -> Result<bool, X509Error> {
        Ok(self.basic_constraints()?.map(|bc| bc.ca).unwrap_or(false))
    }

    fn parse_first_extension<T>(
        &self,
        oid: &str,
        parse: fn(&[u8]) -> Result<T, X509Error>,
    ) -> Result<Option<T>, X509Error> {
        for ext in &self.extensions {
            if ext.oid == oid {
                return parse(&ext.value).map(Some);
            }
        }
        Ok(None)
    }
}

pub fn parse_certificate(der: &[u8]) -> Result<Certificate, X509Error> {
    validate_der_tree(der, MAX_X509_DER_NESTING_DEPTH)?;
    let outer = parse_single(der)?;
    if outer.tag != TAG_SEQUENCE {
        return Err(X509Error::DerParse(DerError::WrongTag {
            expected: TAG_SEQUENCE,
            actual: outer.tag,
        }));
    }

    let mut outer_reader = DerReader::new(outer.content);

    let outer_content_start = outer.content.as_ptr() as usize - der.as_ptr() as usize;
    let tbs_start_in_outer = 0;
    let tbs_start_in_der = outer_content_start + tbs_start_in_outer;

    let tbs_value = outer_reader.read_tlv()?;
    if tbs_value.tag != TAG_SEQUENCE {
        return Err(X509Error::DerParse(DerError::WrongTag {
            expected: TAG_SEQUENCE,
            actual: tbs_value.tag,
        }));
    }

    let tbs_end_in_der = {
        let content_start = tbs_value.content.as_ptr() as usize - der.as_ptr() as usize;
        content_start + tbs_value.content.len()
    };
    let tbs_certificate = der[tbs_start_in_der..tbs_end_in_der].to_vec();

    let tbs = parse_tbs(tbs_value.content)?;

    let sig_alg_value = outer_reader.read_tag(TAG_SEQUENCE)?;
    let sig_alg = parse_algorithm_identifier(&sig_alg_value)?;
    if sig_alg.oid != tbs.signature_algorithm.oid
        || sig_alg.params != tbs.signature_algorithm.params
    {
        return Err(X509Error::InvalidSignature);
    }

    let sig_bs = outer_reader.read_tag(TAG_BIT_STRING)?;
    let (_unused, sig_bytes) = sig_bs.as_bit_string()?;

    Ok(Certificate {
        version: tbs.version,
        serial_number: tbs.serial_number,
        signature_algorithm: sig_alg,
        issuer: tbs.issuer,
        validity: tbs.validity,
        subject: tbs.subject,
        subject_public_key_info: tbs.spki,
        extensions: tbs.extensions,
        tbs_certificate,
        signature_value: sig_bytes.to_vec(),
    })
}

struct TbsFields {
    version: u8,
    serial_number: Vec<u8>,
    signature_algorithm: AlgorithmIdentifier,
    issuer: DistinguishedName,
    validity: Validity,
    subject: DistinguishedName,
    spki: SubjectPublicKeyInfo,
    extensions: Vec<Extension>,
}

fn parse_tbs(tbs_content: &[u8]) -> Result<TbsFields, X509Error> {
    let mut r = DerReader::new(tbs_content);

    let mut version: u8 = 1;
    if let Some(t) = r.peek_tag() {
        if t == 0xA0 {
            let ver_wrap = r.read_tlv()?;
            let inner = ver_wrap.into_reader()?;
            let mut inner = inner;
            let v_val = inner.read_tag(TAG_INTEGER)?;
            let v = v_val.as_i64()?;

            match v {
                0 => version = 1,
                1 => version = 2,
                2 => version = 3,
                _ => return Err(X509Error::UnsupportedVersion(v)),
            }
        }
    }

    let serial = r.read_tag(TAG_INTEGER)?;
    let serial_number = serial.content.to_vec();

    let sig_inner = r.read_tag(TAG_SEQUENCE)?;
    let signature_algorithm = parse_algorithm_identifier(&sig_inner)?;

    let issuer_value = r.read_tag(TAG_SEQUENCE)?;
    let issuer = parse_name(&issuer_value)?;

    let validity_value = r.read_tag(TAG_SEQUENCE)?;
    let validity = parse_validity(&validity_value)?;

    let subject_value = r.read_tag(TAG_SEQUENCE)?;
    let subject = parse_name(&subject_value)?;

    let spki_value = r.read_tag(TAG_SEQUENCE)?;
    let spki = parse_spki(&spki_value)?;

    let mut extensions = Vec::new();
    while let Some(t) = r.peek_tag() {
        let v = r.read_tlv()?;
        match t {
            0x81 => {   }
            0x82 => {   }
            0xA3 => {

                let mut inner = v.into_reader()?;
                let ext_seq = inner.read_tag(TAG_SEQUENCE)?;
                let mut ext_reader = ext_seq.into_reader()?;
                while !ext_reader.is_empty() {
                    let ext_v = ext_reader.read_tag(TAG_SEQUENCE)?;
                    let ext = parse_extension(&ext_v)?;
                    reject_unknown_critical_extension(&ext)?;
                    extensions.push(ext);
                }
            }
            _ => {   }
        }
    }

    Ok(TbsFields {
        version,
        serial_number,
        signature_algorithm,
        issuer,
        validity,
        subject,
        spki,
        extensions,
    })
}

fn parse_algorithm_identifier(v: &DerValue) -> Result<AlgorithmIdentifier, X509Error> {
    let mut r = DerReader::new(v.content);
    let oid_val = r.read_tag(TAG_OID)?;
    let oid = oid_to_string(&oid_val.as_oid()?);
    let params = if r.is_empty() {
        Vec::new()
    } else {
        r.remaining().to_vec()
    };
    Ok(AlgorithmIdentifier { oid, params })
}

fn parse_single_bounded(buf: &[u8]) -> Result<DerValue<'_>, X509Error> {
    validate_der_tree(buf, MAX_X509_DER_NESTING_DEPTH)?;
    parse_single(buf).map_err(X509Error::DerParse)
}

fn reject_embedded_nul(s: &str, label: &str) -> Result<(), X509Error> {
    if s.as_bytes().contains(&0) {
        return Err(X509Error::InvalidExtension(format!(
            "{} contains embedded NUL",
            label
        )));
    }
    Ok(())
}

fn decode_bmp_string(bytes: &[u8]) -> Result<String, X509Error> {
    if bytes.len() % 2 != 0 {
        return Err(X509Error::InvalidExtension(
            "odd-length BMPString in distinguished name".into(),
        ));
    }
    let mut out = String::new();
    for unit in bytes.chunks_exact(2) {
        let code = u16::from_be_bytes([unit[0], unit[1]]) as u32;
        let ch = char::from_u32(code).ok_or_else(|| {
            X509Error::InvalidExtension("invalid BMPString scalar in distinguished name".into())
        })?;
        out.push(ch);
    }
    Ok(out)
}

fn decode_universal_string(bytes: &[u8]) -> Result<String, X509Error> {
    if bytes.len() % 4 != 0 {
        return Err(X509Error::InvalidExtension(
            "misaligned UniversalString in distinguished name".into(),
        ));
    }
    let mut out = String::new();
    for unit in bytes.chunks_exact(4) {
        let code = u32::from_be_bytes([unit[0], unit[1], unit[2], unit[3]]);
        let ch = char::from_u32(code).ok_or_else(|| {
            X509Error::InvalidExtension(
                "invalid UniversalString scalar in distinguished name".into(),
            )
        })?;
        out.push(ch);
    }
    Ok(out)
}

fn decode_name_attribute(v: &DerValue) -> Result<String, X509Error> {
    let s = match v.tag {
        TAG_UTF8_STRING | TAG_PRINTABLE_STRING | TAG_IA5_STRING => {
            v.as_string().map_err(X509Error::DerParse)?.to_string()
        }
        TAG_BMP_STRING => decode_bmp_string(v.content)?,
        TAG_UNIVERSAL_STRING => decode_universal_string(v.content)?,
        TAG_TELETEX_STRING => {
            return Err(X509Error::InvalidExtension(
                "unsupported TeletexString in distinguished name".into(),
            ));
        }
        tag => {
            return Err(X509Error::InvalidExtension(format!(
                "unsupported distinguished-name string tag 0x{:02x}",
                tag
            )));
        }
    };
    reject_embedded_nul(&s, "distinguished name attribute")?;
    Ok(s)
}

fn decode_dns_name(bytes: &[u8]) -> Result<String, X509Error> {
    let s = std::str::from_utf8(bytes)
        .map_err(|_| X509Error::InvalidExtension("invalid dNSName".into()))?;
    reject_embedded_nul(s, "dNSName")?;
    Ok(s.to_ascii_lowercase())
}

fn decode_ip_address(bytes: &[u8]) -> Result<Vec<u8>, X509Error> {
    match bytes.len() {
        4 | 16 => Ok(bytes.to_vec()),
        len => Err(X509Error::InvalidExtension(format!(
            "invalid iPAddress length {}",
            len
        ))),
    }
}

fn parse_name(v: &DerValue) -> Result<DistinguishedName, X509Error> {
    let raw_der = {

        let mut bytes = vec![v.tag];
        append_length(v.content.len(), &mut bytes);
        bytes.extend_from_slice(v.content);
        bytes
    };
    let mut attrs = Vec::new();
    let mut rdn_reader = DerReader::new(v.content);
    while !rdn_reader.is_empty() {
        let rdn = rdn_reader.read_tag(TAG_SET)?;
        let mut atv_reader = DerReader::new(rdn.content);
        while !atv_reader.is_empty() {
            let atv = atv_reader.read_tag(TAG_SEQUENCE)?;
            let mut atv_inner = DerReader::new(atv.content);
            let oid_val = atv_inner.read_tag(TAG_OID)?;
            let oid = oid_to_string(&oid_val.as_oid()?);
            let val_v = atv_inner.read_tlv()?;
            let val_s = decode_name_attribute(&val_v)?;
            attrs.push((oid, val_s));
        }
    }
    Ok(DistinguishedName {
        attributes: attrs,
        raw_der,
    })
}

fn parse_validity(v: &DerValue) -> Result<Validity, X509Error> {
    let mut r = DerReader::new(v.content);
    let nb = r.read_tlv()?;
    let na = r.read_tlv()?;
    if !matches!(nb.tag, TAG_UTC_TIME | TAG_GENERALIZED_TIME) {
        return Err(X509Error::InvalidValidity);
    }
    if !matches!(na.tag, TAG_UTC_TIME | TAG_GENERALIZED_TIME) {
        return Err(X509Error::InvalidValidity);
    }
    Ok(Validity {
        not_before: nb.content.to_vec(),
        not_before_tag: nb.tag,
        not_after: na.content.to_vec(),
        not_after_tag: na.tag,
    })
}

fn parse_spki(v: &DerValue) -> Result<SubjectPublicKeyInfo, X509Error> {
    let raw_der = {
        let mut bytes = vec![v.tag];
        append_length(v.content.len(), &mut bytes);
        bytes.extend_from_slice(v.content);
        bytes
    };
    let mut r = DerReader::new(v.content);
    let alg_v = r.read_tag(TAG_SEQUENCE)?;
    let alg = parse_algorithm_identifier(&alg_v)?;
    let bs = r.read_tag(TAG_BIT_STRING)?;
    let (unused, key_bytes) = bs.as_bit_string()?;
    if unused != 0 {
        return Err(X509Error::InvalidSpki);
    }
    let key = match alg.oid.as_str() {
        OID_RSA_ENCRYPTION => {

            let rsa_seq = parse_single_bounded(key_bytes)?;
            if rsa_seq.tag != TAG_SEQUENCE {
                return Err(X509Error::InvalidSpki);
            }
            let mut rsa_reader = DerReader::new(rsa_seq.content);
            let n_val = rsa_reader.read_tag(TAG_INTEGER)?;
            let n = n_val.as_unsigned_integer()?.to_vec();
            let e_val = rsa_reader.read_tag(TAG_INTEGER)?;
            let e = e_val.as_unsigned_integer()?.to_vec();
            PublicKey::Rsa { n, e }
        }
        OID_EC_PUBLIC_KEY => {

            let params_value = parse_single_bounded(&alg.params)?;
            if params_value.tag != TAG_OID {
                return Err(X509Error::InvalidSpki);
            }
            let curve_arcs = params_value.as_oid()?;
            let curve_oid = oid_to_string(&curve_arcs);

            PublicKey::Ec {
                curve_oid,
                point: key_bytes.to_vec(),
            }
        }
        _ => return Err(X509Error::UnsupportedPubKeyAlg(alg.oid.clone())),
    };
    Ok(SubjectPublicKeyInfo {
        algorithm: alg,
        key,
        raw_der,
    })
}

fn parse_extension(v: &DerValue) -> Result<Extension, X509Error> {
    let mut r = DerReader::new(v.content);
    let oid_val = r.read_tag(TAG_OID)?;
    let oid = oid_to_string(&oid_val.as_oid()?);
    let mut critical = false;
    let next = r.peek_tag();
    if next == Some(TAG_BOOLEAN) {
        let cv = r.read_tag(TAG_BOOLEAN)?;
        critical = cv.as_bool()?;
    }
    let val_v = r.read_tag(TAG_OCTET_STRING)?;
    Ok(Extension {
        oid,
        critical,
        value: val_v.content.to_vec(),
    })
}

fn reject_unknown_critical_extension(ext: &Extension) -> Result<(), X509Error> {
    if ext.critical && !is_recognized_extension_oid(&ext.oid) {
        return Err(X509Error::UnknownCriticalExtension(ext.oid.clone()));
    }
    Ok(())
}

fn is_recognized_extension_oid(oid: &str) -> bool {
    matches!(
        oid,
        OID_SUBJECT_ALT_NAME
            | OID_BASIC_CONSTRAINTS
            | OID_KEY_USAGE
            | OID_EXTENDED_KEY_USAGE
            | OID_NAME_CONSTRAINTS
    )
}

fn parse_basic_constraints(value: &[u8]) -> Result<BasicConstraints, X509Error> {
    let seq = parse_single_bounded(value)?;
    if seq.tag != TAG_SEQUENCE {
        return Err(X509Error::DerParse(DerError::WrongTag {
            expected: TAG_SEQUENCE,
            actual: seq.tag,
        }));
    }
    let mut r = DerReader::new(seq.content);
    let ca = if r.peek_tag() == Some(TAG_BOOLEAN) {
        r.read_tag(TAG_BOOLEAN)?.as_bool()?
    } else {
        false
    };
    let path_len_constraint = if r.peek_tag() == Some(TAG_INTEGER) {
        Some(der_unsigned_to_u64(
            r.read_tag(TAG_INTEGER)?.as_unsigned_integer()?,
        )?)
    } else {
        None
    };
    if !r.is_empty() {
        return Err(X509Error::InvalidExtension(
            "basicConstraints trailing data".into(),
        ));
    }
    Ok(BasicConstraints {
        ca,
        path_len_constraint,
    })
}

fn parse_key_usage(value: &[u8]) -> Result<KeyUsage, X509Error> {
    let bs = parse_single_bounded(value)?;
    let (unused, bytes) = bs.as_bit_string()?;
    let bit = |n: usize| bit_string_has_bit(bytes, unused, n);
    Ok(KeyUsage {
        digital_signature: bit(0),
        non_repudiation: bit(1),
        key_encipherment: bit(2),
        data_encipherment: bit(3),
        key_agreement: bit(4),
        key_cert_sign: bit(5),
        crl_sign: bit(6),
        encipher_only: bit(7),
        decipher_only: bit(8),
    })
}

fn parse_extended_key_usage(value: &[u8]) -> Result<ExtendedKeyUsage, X509Error> {
    let seq = parse_single_bounded(value)?;
    if seq.tag != TAG_SEQUENCE {
        return Err(X509Error::DerParse(DerError::WrongTag {
            expected: TAG_SEQUENCE,
            actual: seq.tag,
        }));
    }
    let mut usages = Vec::new();
    let mut r = DerReader::new(seq.content);
    while !r.is_empty() {
        let oid = r.read_tag(TAG_OID)?;
        usages.push(oid_to_string(&oid.as_oid()?));
    }
    Ok(ExtendedKeyUsage { usages })
}

fn parse_name_constraints(value: &[u8]) -> Result<NameConstraints, X509Error> {
    let seq = parse_single_bounded(value)?;
    if seq.tag != TAG_SEQUENCE {
        return Err(X509Error::DerParse(DerError::WrongTag {
            expected: TAG_SEQUENCE,
            actual: seq.tag,
        }));
    }
    let mut permitted_subtrees = Vec::new();
    let mut excluded_subtrees = Vec::new();
    let mut r = DerReader::new(seq.content);
    while !r.is_empty() {
        let v = r.read_tlv()?;
        match v.tag {
            0xA0 => permitted_subtrees = parse_general_subtrees(v.content)?,
            0xA1 => excluded_subtrees = parse_general_subtrees(v.content)?,
            _ => {
                return Err(X509Error::InvalidExtension(format!(
                    "unexpected nameConstraints field tag 0x{:02x}",
                    v.tag
                )))
            }
        }
    }
    Ok(NameConstraints {
        permitted_subtrees,
        excluded_subtrees,
    })
}

fn parse_general_subtrees(value: &[u8]) -> Result<Vec<GeneralSubtree>, X509Error> {
    let content = match parse_single_bounded(value) {
        Ok(seq) if seq.tag == TAG_SEQUENCE && seq.content.first() == Some(&TAG_SEQUENCE) => {
            seq.content
        }
        Err(X509Error::DerParse(DerError::MaxDepthExceeded)) => {
            return Err(X509Error::DerParse(DerError::MaxDepthExceeded));
        }
        _ => value,
    };
    let mut out = Vec::new();
    let mut r = DerReader::new(content);
    while !r.is_empty() {
        let subtree = r.read_tag(TAG_SEQUENCE)?;
        out.push(parse_general_subtree(subtree.content)?);
    }
    Ok(out)
}

fn parse_general_subtree(value: &[u8]) -> Result<GeneralSubtree, X509Error> {
    let mut r = DerReader::new(value);
    let base = parse_general_name(&r.read_tlv()?)?;
    let mut minimum = 0;
    let mut maximum = None;
    while !r.is_empty() {
        let v = r.read_tlv()?;
        match v.tag {
            0x80 => minimum = der_unsigned_to_u64(v.content)?,
            0x81 => maximum = Some(der_unsigned_to_u64(v.content)?),
            _ => {
                return Err(X509Error::InvalidExtension(format!(
                    "unexpected GeneralSubtree field tag 0x{:02x}",
                    v.tag
                )))
            }
        }
    }
    Ok(GeneralSubtree {
        base,
        minimum,
        maximum,
    })
}

fn parse_general_name(v: &DerValue) -> Result<GeneralName, X509Error> {
    match v.tag {
        0x82 => Ok(GeneralName::DnsName(decode_dns_name(v.content)?)),
        0x87 => Ok(GeneralName::IpAddress(decode_ip_address(v.content)?)),
        _ => Ok(GeneralName::Other {
            tag: v.tag,
            value: v.content.to_vec(),
        }),
    }
}

fn bit_string_has_bit(bytes: &[u8], unused: u8, n: usize) -> bool {
    let total_bits = bytes.len() * 8;
    if n >= total_bits.saturating_sub(unused as usize) {
        return false;
    }
    let byte = bytes[n / 8];
    let mask = 0x80u8 >> (n % 8);
    (byte & mask) != 0
}

fn der_unsigned_to_u64(bytes: &[u8]) -> Result<u64, X509Error> {
    if bytes.is_empty() {
        return Err(X509Error::InvalidExtension("empty integer".into()));
    }
    if bytes.len() > 8 {
        return Err(X509Error::InvalidExtension("integer too large".into()));
    }
    let mut out = 0u64;
    for &b in bytes {
        out = (out << 8) | b as u64;
    }
    Ok(out)
}

fn append_length(n: usize, out: &mut Vec<u8>) {
    if n < 0x80 {
        out.push(n as u8);
    } else {
        let mut len_bytes = Vec::new();
        let mut tmp = n;
        while tmp > 0 {
            len_bytes.push((tmp & 0xFF) as u8);
            tmp >>= 8;
        }
        len_bytes.reverse();
        out.push(0x80 | (len_bytes.len() as u8));
        out.extend_from_slice(&len_bytes);
    }
}

pub fn verify_signature(
    cert: &Certificate,
    issuer_spki: &SubjectPublicKeyInfo,
) -> Result<(), X509Error> {
    let _profile = std::env::var("CRUFT_TLS_PROFILE").is_ok();
    let _t0 = std::time::Instant::now();
    let sig_oid = cert.signature_algorithm.oid.as_str();
    match sig_oid {
        OID_SHA256_WITH_RSA | OID_SHA384_WITH_RSA | OID_SHA512_WITH_RSA => {
            let (n, e) = match &issuer_spki.key {
                PublicKey::Rsa { n, e } => (n, e),
                _ => return Err(X509Error::UnsupportedSigAlg(sig_oid.into())),
            };
            let (hash, hash_name) = compute_hash_for_rsa(sig_oid, &cert.tbs_certificate)?;
            let r = rusty_web_crypto::rsa_pkcs1_v15_verify(
                n,
                e,
                &hash,
                &cert.signature_value,
                hash_name,
            )
            .map_err(X509Error::CryptoFail);
            if _profile {
                eprintln!(
                    "[wc-ext-14] verify_signature RSA {} → {:?} in {:?}",
                    sig_oid,
                    r.is_ok(),
                    _t0.elapsed()
                );
            }
            r
        }
        OID_ECDSA_WITH_SHA256 | OID_ECDSA_WITH_SHA384 | OID_ECDSA_WITH_SHA512 => {
            let (curve_oid, point) = match &issuer_spki.key {
                PublicKey::Ec { curve_oid, point } => (curve_oid, point),
                _ => return Err(X509Error::UnsupportedSigAlg(sig_oid.into())),
            };
            let curve = match curve_oid.as_str() {
                OID_P256_CURVE => rusty_web_crypto::curve_p256(),
                OID_P384_CURVE => rusty_web_crypto::curve_p384(),
                _ => return Err(X509Error::UnsupportedPubKeyAlg(curve_oid.clone())),
            };

            if point.is_empty() || point[0] != 0x04 {
                return Err(X509Error::InvalidSpki);
            }
            let coord = curve.coord_bytes;
            if point.len() != 1 + 2 * coord {
                return Err(X509Error::InvalidSpki);
            }
            let qx = &point[1..1 + coord];
            let qy = &point[1 + coord..];

            let sig_seq = parse_single_bounded(&cert.signature_value)?;
            if sig_seq.tag != TAG_SEQUENCE {
                return Err(X509Error::InvalidSignature);
            }
            let mut sig_reader = DerReader::new(sig_seq.content);
            let r_val = sig_reader.read_tag(TAG_INTEGER)?;
            let s_val = sig_reader.read_tag(TAG_INTEGER)?;
            let r = r_val.as_unsigned_integer()?;
            let s = s_val.as_unsigned_integer()?;

            let mut sig_raw = vec![0u8; 2 * coord];
            sig_raw[coord - r.len()..coord].copy_from_slice(r);
            sig_raw[2 * coord - s.len()..].copy_from_slice(s);
            let hash = match sig_oid {
                OID_ECDSA_WITH_SHA256 => {
                    rusty_web_crypto::digest_sha256(&cert.tbs_certificate).to_vec()
                }
                OID_ECDSA_WITH_SHA384 => {
                    rusty_web_crypto::digest_sha384(&cert.tbs_certificate).to_vec()
                }
                OID_ECDSA_WITH_SHA512 => {
                    rusty_web_crypto::digest_sha512(&cert.tbs_certificate).to_vec()
                }
                _ => unreachable!(),
            };
            let r = rusty_web_crypto::ecdsa_verify(&curve, qx, qy, &hash, &sig_raw)
                .map_err(X509Error::CryptoFail);
            if _profile {
                eprintln!(
                    "[wc-ext-14] verify_signature ECDSA {} → {:?} in {:?}",
                    sig_oid,
                    r.is_ok(),
                    _t0.elapsed()
                );
            }
            r
        }
        _ => Err(X509Error::UnsupportedSigAlg(sig_oid.into())),
    }
}

fn compute_hash_for_rsa(sig_oid: &str, tbs: &[u8]) -> Result<(Vec<u8>, &'static str), X509Error> {
    match sig_oid {
        OID_SHA256_WITH_RSA => Ok((rusty_web_crypto::digest_sha256(tbs).to_vec(), "SHA-256")),
        OID_SHA384_WITH_RSA => Ok((rusty_web_crypto::digest_sha384(tbs).to_vec(), "SHA-384")),
        OID_SHA512_WITH_RSA => Ok((rusty_web_crypto::digest_sha512(tbs).to_vec(), "SHA-512")),
        _ => Err(X509Error::UnsupportedSigAlg(sig_oid.into())),
    }
}

const BEGIN: &str = "-----BEGIN CERTIFICATE-----";
const END: &str = "-----END CERTIFICATE-----";

pub fn pem_to_der(pem: &str) -> Result<Vec<u8>, X509Error> {
    let begin_pos = pem.find(BEGIN).ok_or(X509Error::PemBadHeader)?;
    let after_begin = begin_pos + BEGIN.len();
    let end_pos = pem[after_begin..]
        .find(END)
        .ok_or(X509Error::PemBadHeader)?;
    let b64_block: String = pem[after_begin..after_begin + end_pos]
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    base64_decode(&b64_block)
}

pub fn pem_all_to_der(pem: &str) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut cursor = pem;
    while let Some(b) = cursor.find(BEGIN) {
        let after_begin = b + BEGIN.len();
        let rest = &cursor[after_begin..];
        if let Some(e) = rest.find(END) {
            let b64: String = rest[..e].chars().filter(|c| !c.is_whitespace()).collect();
            if let Ok(der) = base64_decode(&b64) {
                out.push(der);
            }
            cursor = &rest[e + END.len()..];
        } else {
            break;
        }
    }
    out
}

fn base64_decode(s: &str) -> Result<Vec<u8>, X509Error> {
    const BAD: u8 = 255;
    let mut table = [BAD; 256];
    for (i, c) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
        .iter()
        .enumerate()
    {
        table[*c as usize] = i as u8;
    }
    let mut out = Vec::with_capacity((s.len() * 3) / 4);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 3 < bytes.len() {
        let a = table[bytes[i] as usize];
        let b = table[bytes[i + 1] as usize];
        let c = table[bytes[i + 2] as usize];
        let d = table[bytes[i + 3] as usize];
        i += 4;
        if a == BAD || b == BAD {
            return Err(X509Error::PemBadBase64);
        }
        if bytes[i - 4 + 2] == b'=' {

            out.push((a << 2) | (b >> 4));
            break;
        }
        if c == BAD {
            return Err(X509Error::PemBadBase64);
        }
        if bytes[i - 4 + 3] == b'=' {
            out.push((a << 2) | (b >> 4));
            out.push((b << 4) | (c >> 2));
            break;
        }
        if d == BAD {
            return Err(X509Error::PemBadBase64);
        }
        out.push((a << 2) | (b >> 4));
        out.push((b << 4) | (c >> 2));
        out.push((c << 6) | d);
    }
    Ok(out)
}
