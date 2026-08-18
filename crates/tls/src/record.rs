
use std::convert::TryFrom;

pub const MAX_PLAINTEXT_LEN: usize = 1 << 14;
pub const MAX_CIPHERTEXT_LEN: usize = (1 << 14) + 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22,
    ApplicationData = 23,

    Heartbeat = 24,
}

impl TryFrom<u8> for ContentType {
    type Error = TlsError;
    fn try_from(b: u8) -> Result<Self, Self::Error> {
        Ok(match b {
            20 => ContentType::ChangeCipherSpec,
            21 => ContentType::Alert,
            22 => ContentType::Handshake,
            23 => ContentType::ApplicationData,
            24 => ContentType::Heartbeat,
            _ => return Err(TlsError::UnknownContentType(b)),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolVersion(pub u16);

impl ProtocolVersion {
    pub const TLS_1_2: ProtocolVersion = ProtocolVersion(0x0303);
    pub const TLS_1_3: ProtocolVersion = ProtocolVersion(0x0304);

    pub const LEGACY: ProtocolVersion = ProtocolVersion(0x0303);
}

#[derive(Debug, Clone)]
pub struct TlsRecord {
    pub content_type: ContentType,
    pub version: ProtocolVersion,
    pub fragment: Vec<u8>,
}

#[derive(Debug, Clone)]
pub enum TlsError {
    UnexpectedEnd,
    UnknownContentType(u8),
    FragmentTooLong(usize),
    InvalidAlert,
    TrustAnchorNotFound,
    NoIssuerFound,
    SignatureFail(String),
    ValidityExpired,
    CertificateNameMismatch(String),
    CertificatePurposeMismatch,
    SelfSignedNotInTrust,
    StoreLoad(String),
    X509(X509Error),

    WouldBlock,

    CloseNotify,
}

use rusty_x509::X509Error;
impl From<X509Error> for TlsError {
    fn from(e: X509Error) -> Self {
        TlsError::X509(e)
    }
}

impl std::fmt::Display for TlsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsError::UnexpectedEnd => write!(f, "unexpected end of TLS record"),
            TlsError::UnknownContentType(b) => write!(f, "unknown ContentType {}", b),
            TlsError::FragmentTooLong(n) => write!(f, "fragment too long ({} bytes)", n),
            TlsError::InvalidAlert => write!(f, "invalid Alert encoding"),
            TlsError::TrustAnchorNotFound => write!(f, "trust anchor not found in store"),
            TlsError::NoIssuerFound => write!(f, "no issuer found for certificate"),
            TlsError::SignatureFail(s) => write!(f, "signature verification failed: {}", s),
            TlsError::ValidityExpired => write!(f, "certificate validity expired"),
            TlsError::CertificateNameMismatch(host) => {
                write!(f, "certificate name does not match host {}", host)
            }
            TlsError::CertificatePurposeMismatch => {
                write!(f, "certificate is not valid for TLS server authentication")
            }
            TlsError::SelfSignedNotInTrust => {
                write!(f, "self-signed certificate not present in trust store")
            }
            TlsError::StoreLoad(s) => write!(f, "trust store load failed: {}", s),
            TlsError::X509(e) => write!(f, "X.509: {}", e),
            TlsError::WouldBlock => write!(f, "transport would block"),
            TlsError::CloseNotify => write!(f, "peer sent close_notify (clean shutdown)"),
        }
    }
}

impl std::error::Error for TlsError {}

pub fn classify_alert(bytes: &[u8]) -> TlsError {
    if bytes.len() < 2 {
        return TlsError::InvalidAlert;
    }
    let level = bytes[0];
    let description = bytes[1];
    if level == AlertLevel::Warning as u8 && description == AlertDescription::CLOSE_NOTIFY.0 {
        TlsError::CloseNotify
    } else {
        TlsError::SignatureFail(format!("server alert: [{}, {}]", level, description))
    }
}

pub fn encode_record(record: &TlsRecord) -> Result<Vec<u8>, TlsError> {
    if record.fragment.len() > MAX_CIPHERTEXT_LEN {
        return Err(TlsError::FragmentTooLong(record.fragment.len()));
    }
    let mut out = Vec::with_capacity(5 + record.fragment.len());
    out.push(record.content_type as u8);
    let v = record.version.0;
    out.push((v >> 8) as u8);
    out.push((v & 0xFF) as u8);
    let len = record.fragment.len() as u16;
    out.push((len >> 8) as u8);
    out.push((len & 0xFF) as u8);
    out.extend_from_slice(&record.fragment);
    Ok(out)
}

pub fn decode_record(buf: &[u8]) -> Result<(TlsRecord, usize), TlsError> {
    if buf.len() < 5 {
        return Err(TlsError::UnexpectedEnd);
    }
    let content_type = ContentType::try_from(buf[0])?;
    let version = ProtocolVersion(((buf[1] as u16) << 8) | (buf[2] as u16));
    let length = (((buf[3] as u16) << 8) | (buf[4] as u16)) as usize;
    if length > MAX_CIPHERTEXT_LEN {
        return Err(TlsError::FragmentTooLong(length));
    }
    if buf.len() < 5 + length {
        return Err(TlsError::UnexpectedEnd);
    }
    let fragment = buf[5..5 + length].to_vec();
    Ok((
        TlsRecord {
            content_type,
            version,
            fragment,
        },
        5 + length,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertLevel {
    Warning = 1,
    Fatal = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlertDescription(pub u8);

impl AlertDescription {
    pub const CLOSE_NOTIFY: AlertDescription = AlertDescription(0);
    pub const UNEXPECTED_MESSAGE: AlertDescription = AlertDescription(10);
    pub const BAD_RECORD_MAC: AlertDescription = AlertDescription(20);
    pub const RECORD_OVERFLOW: AlertDescription = AlertDescription(22);
    pub const HANDSHAKE_FAILURE: AlertDescription = AlertDescription(40);
    pub const BAD_CERTIFICATE: AlertDescription = AlertDescription(42);
    pub const UNSUPPORTED_CERTIFICATE: AlertDescription = AlertDescription(43);
    pub const CERTIFICATE_REVOKED: AlertDescription = AlertDescription(44);
    pub const CERTIFICATE_EXPIRED: AlertDescription = AlertDescription(45);
    pub const CERTIFICATE_UNKNOWN: AlertDescription = AlertDescription(46);
    pub const ILLEGAL_PARAMETER: AlertDescription = AlertDescription(47);
    pub const UNKNOWN_CA: AlertDescription = AlertDescription(48);
    pub const ACCESS_DENIED: AlertDescription = AlertDescription(49);
    pub const DECODE_ERROR: AlertDescription = AlertDescription(50);
    pub const DECRYPT_ERROR: AlertDescription = AlertDescription(51);
    pub const PROTOCOL_VERSION: AlertDescription = AlertDescription(70);
    pub const INSUFFICIENT_SECURITY: AlertDescription = AlertDescription(71);
    pub const INTERNAL_ERROR: AlertDescription = AlertDescription(80);
    pub const INAPPROPRIATE_FALLBACK: AlertDescription = AlertDescription(86);
    pub const USER_CANCELED: AlertDescription = AlertDescription(90);
    pub const MISSING_EXTENSION: AlertDescription = AlertDescription(109);
    pub const UNSUPPORTED_EXTENSION: AlertDescription = AlertDescription(110);
    pub const UNRECOGNIZED_NAME: AlertDescription = AlertDescription(112);
    pub const BAD_CERTIFICATE_STATUS_RESPONSE: AlertDescription = AlertDescription(113);
    pub const UNKNOWN_PSK_IDENTITY: AlertDescription = AlertDescription(115);
    pub const CERTIFICATE_REQUIRED: AlertDescription = AlertDescription(116);
    pub const NO_APPLICATION_PROTOCOL: AlertDescription = AlertDescription(120);
}

pub fn encode_alert(level: AlertLevel, description: AlertDescription) -> Vec<u8> {
    vec![level as u8, description.0]
}

pub fn decode_alert(buf: &[u8]) -> Result<(AlertLevel, AlertDescription), TlsError> {
    if buf.len() != 2 {
        return Err(TlsError::InvalidAlert);
    }
    let level = match buf[0] {
        1 => AlertLevel::Warning,
        2 => AlertLevel::Fatal,
        _ => return Err(TlsError::InvalidAlert),
    };
    Ok((level, AlertDescription(buf[1])))
}
