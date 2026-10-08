//! Minimal, zero-dependency CBOR decoder for Ordinals metadata & properties.
//!
//! Handles CBOR (RFC 8949):
//! - Major 0: Unsigned integer
//! - Major 1: Negative integer
//! - Major 2: Byte string (definite & indefinite)
//! - Major 3: UTF-8 text string (definite & indefinite)
//! - Major 4: Array
//! - Major 5: Map
//! - Major 6: Tag
//! - Major 7: Simple values (false, true, null) & break code

#[derive(Debug, Clone, PartialEq)]
pub enum CborValue {
    Unsigned(u64),
    Negative(i64),
    Bytes(Vec<u8>),
    Text(String),
    Array(Vec<CborValue>),
    Map(Vec<(CborValue, CborValue)>),
    Tag(u64, Box<CborValue>),
    Bool(bool),
    Null,
    Undefined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CborError {
    UnexpectedEof,
    InvalidUtf8,
    UnsupportedAdditionalInfo(u8),
    RecursionLimitExceeded,
    TrailingBytes,
}

impl std::fmt::Display for CborError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof => write!(f, "cbor_unexpected_eof"),
            Self::InvalidUtf8 => write!(f, "cbor_invalid_utf8"),
            Self::UnsupportedAdditionalInfo(info) => write!(f, "cbor_unsupported_info_{}", info),
            Self::RecursionLimitExceeded => write!(f, "cbor_recursion_limit"),
            Self::TrailingBytes => write!(f, "cbor_trailing_bytes"),
        }
    }
}

pub struct CborDecoder<'a> {
    data: &'a [u8],
    offset: usize,
    depth: usize,
}

const MAX_DEPTH: usize = 64;

impl<'a> CborDecoder<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0, depth: 0 }
    }

    fn read_byte(&mut self) -> Result<u8, CborError> {
        if self.offset >= self.data.len() {
            return Err(CborError::UnexpectedEof);
        }
        let b = self.data[self.offset];
        self.offset += 1;
        Ok(b)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], CborError> {
        if self.offset + len > self.data.len() {
            return Err(CborError::UnexpectedEof);
        }
        let slice = &self.data[self.offset..self.offset + len];
        self.offset += len;
        Ok(slice)
    }

    fn read_length(&mut self, additional_info: u8) -> Result<u64, CborError> {
        match additional_info {
            0..=23 => Ok(additional_info as u64),
            24 => Ok(self.read_byte()? as u64),
            25 => {
                let b = self.read_bytes(2)?;
                Ok(u16::from_be_bytes([b[0], b[1]]) as u64)
            }
            26 => {
                let b = self.read_bytes(4)?;
                Ok(u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as u64)
            }
            27 => {
                let b = self.read_bytes(8)?;
                Ok(u64::from_be_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]))
            }
            other => Err(CborError::UnsupportedAdditionalInfo(other)),
        }
    }

    pub fn decode_value(&mut self) -> Result<CborValue, CborError> {
        if self.depth > MAX_DEPTH {
            return Err(CborError::RecursionLimitExceeded);
        }
        self.depth += 1;
        let res = self.decode_inner();
        self.depth -= 1;
        res
    }

    fn decode_inner(&mut self) -> Result<CborValue, CborError> {
        let initial_byte = self.read_byte()?;
        let major = initial_byte >> 5;
        let additional = initial_byte & 0x1f;

        match major {
            0 => {
                let val = self.read_length(additional)?;
                Ok(CborValue::Unsigned(val))
            }
            1 => {
                let val = self.read_length(additional)?;
                // -1 - val
                let neg = -1i64.checked_sub(val as i64).unwrap_or(i64::MIN);
                Ok(CborValue::Negative(neg))
            }
            2 => {
                if additional == 31 {
                    // Indefinite length byte string
                    let mut bytes = Vec::new();
                    loop {
                        if self.offset >= self.data.len() {
                            return Err(CborError::UnexpectedEof);
                        }
                        if self.data[self.offset] == 0xff {
                            self.offset += 1;
                            break;
                        }
                        match self.decode_value()? {
                            CborValue::Bytes(chunk) => bytes.extend_from_slice(&chunk),
                            _ => return Err(CborError::UnsupportedAdditionalInfo(additional)),
                        }
                    }
                    Ok(CborValue::Bytes(bytes))
                } else {
                    let len = self.read_length(additional)? as usize;
                    let slice = self.read_bytes(len)?;
                    Ok(CborValue::Bytes(slice.to_vec()))
                }
            }
            3 => {
                if additional == 31 {
                    // Indefinite length text string
                    let mut s = String::new();
                    loop {
                        if self.offset >= self.data.len() {
                            return Err(CborError::UnexpectedEof);
                        }
                        if self.data[self.offset] == 0xff {
                            self.offset += 1;
                            break;
                        }
                        match self.decode_value()? {
                            CborValue::Text(chunk) => s.push_str(&chunk),
                            _ => return Err(CborError::UnsupportedAdditionalInfo(additional)),
                        }
                    }
                    Ok(CborValue::Text(s))
                } else {
                    let len = self.read_length(additional)? as usize;
                    let slice = self.read_bytes(len)?;
                    let s = std::str::from_utf8(slice).map_err(|_| CborError::InvalidUtf8)?;
                    Ok(CborValue::Text(s.to_string()))
                }
            }
            4 => {
                if additional == 31 {
                    let mut items = Vec::new();
                    loop {
                        if self.offset >= self.data.len() {
                            return Err(CborError::UnexpectedEof);
                        }
                        if self.data[self.offset] == 0xff {
                            self.offset += 1;
                            break;
                        }
                        items.push(self.decode_value()?);
                    }
                    Ok(CborValue::Array(items))
                } else {
                    let count = self.read_length(additional)? as usize;
                    let mut items = Vec::with_capacity(count.min(1024));
                    for _ in 0..count {
                        items.push(self.decode_value()?);
                    }
                    Ok(CborValue::Array(items))
                }
            }
            5 => {
                if additional == 31 {
                    let mut pairs = Vec::new();
                    loop {
                        if self.offset >= self.data.len() {
                            return Err(CborError::UnexpectedEof);
                        }
                        if self.data[self.offset] == 0xff {
                            self.offset += 1;
                            break;
                        }
                        let k = self.decode_value()?;
                        let v = self.decode_value()?;
                        pairs.push((k, v));
                    }
                    Ok(CborValue::Map(pairs))
                } else {
                    let count = self.read_length(additional)? as usize;
                    let mut pairs = Vec::with_capacity(count.min(1024));
                    for _ in 0..count {
                        let k = self.decode_value()?;
                        let v = self.decode_value()?;
                        pairs.push((k, v));
                    }
                    Ok(CborValue::Map(pairs))
                }
            }
            6 => {
                let tag = self.read_length(additional)?;
                let inner = self.decode_value()?;
                Ok(CborValue::Tag(tag, Box::new(inner)))
            }
            7 => match additional {
                20 => Ok(CborValue::Bool(false)),
                21 => Ok(CborValue::Bool(true)),
                22 => Ok(CborValue::Null),
                23 => Ok(CborValue::Undefined),
                other => Err(CborError::UnsupportedAdditionalInfo(other)),
            },
            other => Err(CborError::UnsupportedAdditionalInfo(other)),
        }
    }
}

impl CborValue {
    pub fn parse(data: &[u8]) -> Result<Self, CborError> {
        let mut decoder = CborDecoder::new(data);
        let val = decoder.decode_value()?;
        Ok(val)
    }

    pub fn get_str_key(&self, key: &str) -> Option<&CborValue> {
        if let CborValue::Map(pairs) = self {
            for (k, v) in pairs {
                if let CborValue::Text(ref s) = k {
                    if s == key {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    pub fn get_u64_key(&self, key: u64) -> Option<&CborValue> {
        if let CborValue::Map(pairs) = self {
            for (k, v) in pairs {
                if let CborValue::Unsigned(u) = k {
                    if *u == key {
                        return Some(v);
                    }
                }
            }
        }
        None
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            CborValue::Bytes(b) => Some(b),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            CborValue::Text(s) => Some(s),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            CborValue::Unsigned(u) => serde_json::json!(u),
            CborValue::Negative(n) => serde_json::json!(n),
            CborValue::Bytes(b) => serde_json::json!(hex::encode(b)),
            CborValue::Text(s) => serde_json::json!(s),
            CborValue::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|item| item.to_json()).collect())
            }
            CborValue::Map(pairs) => {
                let mut map = serde_json::Map::new();
                for (k, v) in pairs {
                    let key_str = match k {
                        CborValue::Text(s) => s.clone(),
                        CborValue::Unsigned(u) => u.to_string(),
                        CborValue::Negative(n) => n.to_string(),
                        other => format!("{:?}", other),
                    };
                    map.insert(key_str, v.to_json());
                }
                serde_json::Value::Object(map)
            }
            CborValue::Tag(_t, inner) => inner.to_json(),
            CborValue::Bool(b) => serde_json::json!(b),
            CborValue::Null | CborValue::Undefined => serde_json::Value::Null,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cbor_decode_uint_and_text() {
        // map { 1: "hello" } -> a1 01 65 68656c6c6f
        let raw = hex::decode("a1016568656c6c6f").unwrap();
        let val = CborValue::parse(&raw).unwrap();
        assert_eq!(val.get_u64_key(1).unwrap().as_str(), Some("hello"));
    }

    #[test]
    fn test_cbor_decode_tag5_koala() {
        // Tag 5 of Koala: a1 6e 63726332315f7661756c745f7631 58 dc <220 bytes>
        let mut raw = hex::decode("a16e63726332315f7661756c745f763158dc").unwrap();
        raw.extend(vec![0x42; 220]);
        let val = CborValue::parse(&raw).unwrap();
        let s_bytes = val.get_str_key("crc21_vault_v1").unwrap().as_bytes().unwrap();
        assert_eq!(s_bytes.len(), 220);
        assert_eq!(s_bytes[0], 0x42);
    }
}
