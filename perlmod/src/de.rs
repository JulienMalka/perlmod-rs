use serde::de::{self, DeserializeOwned, DeserializeSeed, MapAccess, SeqAccess, Visitor};

use crate::error::Error;
use crate::scalar::Type;
use crate::Value;
use crate::{array, ffi, hash};

pub struct Deserializer {
    input: Value,
    option_allowed: bool,
}

pub fn from_value<T>(input: Value) -> Result<T, Error>
where
    T: DeserializeOwned,
{
    let mut deserializer = Deserializer::from_value(input);
    let out = T::deserialize(&mut deserializer)?;
    Ok(out)
}

impl Deserializer {
    pub fn from_value(input: Value) -> Self {
        Deserializer {
            input,
            option_allowed: true,
        }
    }

    pub fn from_some_value(input: Value) -> Self {
        Deserializer {
            input,
            option_allowed: false,
        }
    }

    fn deref_current(&mut self) -> Result<(), Error> {
        while let Value::Reference(_) = &self.input {
            self.input = self.input.dereference().ok_or_else(|| {
                Error::new("failed to dereference a reference while deserializing")
            })?;
        }
        Ok(())
    }

    fn sanity_check(&mut self) -> Result<(), Error> {
        if let Value::Scalar(value) = &self.input {
            match value.ty() {
                Type::Scalar(_) => Ok(()),
                Type::Other(_) => Error::fail("cannot deserialize weird magic perl values"),
                // These are impossible as they are all handled by different Value enum types:
                Type::Reference => Error::fail("Value::Scalar: containing a reference"),
                Type::Array => Error::fail("Value::Scalar: containing an array"),
                Type::Hash => Error::fail("Value::Scalar: containing a hash"),
            }
        } else {
            Ok(())
        }
    }

    fn get(&mut self) -> Result<&Value, Error> {
        self.deref_current()?;
        self.sanity_check()?;
        Ok(&self.input)
    }
}

impl Deserializer {
    /// deserialize_any, preferring a string value
    fn deserialize_any_string<'de, V>(&mut self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.get()? {
            Value::Scalar(value) => match value.ty() {
                Type::Scalar(flags) => {
                    use crate::scalar::Flags;

                    if flags.contains(Flags::STRING) {
                        visitor.visit_str(value.pv_utf8())
                    } else if flags.contains(Flags::DOUBLE) {
                        visitor.visit_f64(value.nv())
                    } else if flags.contains(Flags::INTEGER) {
                        visitor.visit_i64(value.iv() as i64)
                    } else {
                        visitor.visit_unit()
                    }
                }
                _ => unreachable!(),
            },
            Value::Hash(value) => visitor.visit_map(HashAccess::new(value)),
            Value::Array(value) => visitor.visit_seq(ArrayAccess::new(value)),
            Value::Reference(_) => unreachable!(),
        }
    }

    /// deserialize_any, preferring an integer value
    fn deserialize_any_iv<'de, V>(&mut self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.get()? {
            Value::Scalar(value) => match value.ty() {
                Type::Scalar(flags) => {
                    use crate::scalar::Flags;

                    if flags.contains(Flags::INTEGER) {
                        visitor.visit_i64(value.iv() as i64)
                    } else if flags.contains(Flags::DOUBLE) {
                        visitor.visit_f64(value.nv())
                    } else if flags.contains(Flags::STRING) {
                        visitor.visit_str(value.pv_utf8())
                    } else {
                        visitor.visit_unit()
                    }
                }
                _ => unreachable!(),
            },
            Value::Hash(value) => visitor.visit_map(HashAccess::new(value)),
            Value::Array(value) => visitor.visit_seq(ArrayAccess::new(value)),
            Value::Reference(_) => unreachable!(),
        }
    }

    /// deserialize_any, preferring a float value
    fn deserialize_any_nv<'de, V>(&mut self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.get()? {
            Value::Scalar(value) => match value.ty() {
                Type::Scalar(flags) => {
                    use crate::scalar::Flags;

                    if flags.contains(Flags::DOUBLE) {
                        visitor.visit_f64(value.nv())
                    } else if flags.contains(Flags::INTEGER) {
                        visitor.visit_i64(value.iv() as i64)
                    } else if flags.contains(Flags::STRING) {
                        visitor.visit_str(value.pv_utf8())
                    } else {
                        visitor.visit_unit()
                    }
                }
                _ => unreachable!(),
            },
            Value::Hash(value) => visitor.visit_map(HashAccess::new(value)),
            Value::Array(value) => visitor.visit_seq(ArrayAccess::new(value)),
            Value::Reference(_) => unreachable!(),
        }
    }
}

impl<'de, 'a> de::Deserializer<'de> for &'a mut Deserializer {
    type Error = Error;

    fn deserialize_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_string(visitor)
    }

    fn deserialize_bool<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.get()? {
            Value::Scalar(value) => match value.ty() {
                Type::Scalar(flags) => {
                    use crate::scalar::Flags;

                    if flags.is_empty() || flags.intersects(Flags::INTEGER | Flags::DOUBLE) {
                        visitor.visit_bool(unsafe { ffi::RSPL_SvTRUE(value.sv()) })
                    } else {
                        Error::fail("expected bool value")
                    }
                }
                _ => unreachable!(),
            },
            Value::Hash(value) => visitor.visit_map(HashAccess::new(value)),
            Value::Array(value) => visitor.visit_seq(ArrayAccess::new(value)),
            Value::Reference(_) => unreachable!(),
        }
    }

    fn deserialize_i8<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_u8<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_i16<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_u16<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_i32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_u32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_i64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_u64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_iv(visitor)
    }

    fn deserialize_f32<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_nv(visitor)
    }

    fn deserialize_f64<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any_nv(visitor)
    }

    fn deserialize_char<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.get()? {
            Value::Scalar(value) => match value.ty() {
                Type::Scalar(flags) => {
                    use crate::scalar::Flags;

                    if flags.contains(Flags::INTEGER) {
                        let c = value.iv();
                        if c < 0x100 {
                            visitor.visit_char(c as u8 as char)
                        } else {
                            visitor.visit_i64(c as i64)
                        }
                    } else if flags.contains(Flags::DOUBLE) {
                        visitor.visit_f64(value.nv())
                    } else if flags.contains(Flags::STRING) {
                        let s = value.pv_utf8();
                        let mut chars = s.chars();
                        match chars.next() {
                            Some(ch) if chars.next().is_none() => visitor.visit_char(ch),
                            _ => visitor.visit_str(value.pv_utf8()),
                        }
                    } else {
                        visitor.visit_unit()
                    }
                }
                _ => unreachable!(),
            },
            Value::Hash(value) => visitor.visit_map(HashAccess::new(value)),
            Value::Array(value) => visitor.visit_seq(ArrayAccess::new(value)),
            Value::Reference(_) => unreachable!(),
        }
    }

    fn deserialize_str<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_string<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_bytes<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        match self.get()? {
            Value::Scalar(value) => match value.ty() {
                Type::Scalar(flags) => {
                    use crate::scalar::Flags;

                    if flags.contains(Flags::STRING) {
                        visitor.visit_bytes(value.pv_bytes())
                    } else if flags.contains(Flags::DOUBLE) {
                        visitor.visit_f64(value.nv())
                    } else if flags.contains(Flags::INTEGER) {
                        visitor.visit_i64(value.iv() as i64)
                    } else {
                        visitor.visit_unit()
                    }
                }
                _ => unreachable!(),
            },
            Value::Hash(value) => visitor.visit_map(HashAccess::new(value)),
            Value::Array(value) => visitor.visit_seq(ArrayAccess::new(value)),
            Value::Reference(_) => unreachable!(),
        }
    }

    fn deserialize_byte_buf<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        if self.option_allowed {
            if let Value::Scalar(value) = self.get()? {
                if let Type::Scalar(flags) = value.ty() {
                    if flags.is_empty() {
                        return visitor.visit_none();
                    }
                }
            }
            self.option_allowed = false;
            let res = visitor.visit_some(&mut *self);
            self.option_allowed = true;
            res
        } else {
            self.deserialize_any(visitor)
        }
    }

    fn deserialize_unit<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_unit_struct<V>(self, _name: &'static str, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_newtype_struct<V>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_seq<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple<V>(self, _len: usize, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_tuple_struct<V>(
        self,
        _name: &'static str,
        _len: usize,
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_map<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_struct<V>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V>(
        self,
        _name: &'static str,
        _variants: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_identifier<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V>(self, visitor: V) -> Result<V::Value, Error>
    where
        V: Visitor<'de>,
    {
        self.deserialize_any(visitor)
    }
}

pub struct HashAccess<'a> {
    hash: &'a hash::Hash,
    entry: *mut ffi::HE,
    finished: bool,
    at_value: bool,
}

impl<'a> HashAccess<'a> {
    pub fn new(value: &'a hash::Hash) -> Self {
        drop(value.shared_iter()); // reset iterator
        Self {
            hash: &value,
            entry: std::ptr::null_mut(),
            finished: false,
            at_value: false,
        }
    }
}

impl<'de, 'a> MapAccess<'de> for HashAccess<'a> {
    type Error = Error;

    fn next_key_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: DeserializeSeed<'de>,
    {
        if self.finished {
            return Ok(None);
        }

        if self.entry.is_null() {
            self.entry = unsafe { ffi::RSPL_hv_iternext(self.hash.hv()) };
            if self.entry.is_null() {
                self.finished = true;
                return Ok(None);
            }
        } else if self.at_value {
            return Error::fail("map access value skipped");
        }

        self.at_value = true;

        let key = unsafe { Value::from_raw_ref(ffi::RSPL_hv_iterkeysv(self.entry)) };
        seed.deserialize(&mut Deserializer::from_value(key))
            .map(Some)
    }

    fn next_value_seed<V>(&mut self, seed: V) -> Result<V::Value, Error>
    where
        V: DeserializeSeed<'de>,
    {
        if self.finished {
            return Error::fail("map access value requested after end");
        }

        if self.entry.is_null() || !self.at_value {
            return Error::fail("map access key skipped");
        }

        self.at_value = false;

        let value =
            unsafe { Value::from_raw_ref(ffi::RSPL_hv_iterval(self.hash.hv(), self.entry)) };
        self.entry = std::ptr::null_mut();

        seed.deserialize(&mut Deserializer::from_value(value))
    }
}

pub struct ArrayAccess<'a> {
    iter: array::Iter<'a>,
}

impl<'a> ArrayAccess<'a> {
    pub fn new(value: &'a array::Array) -> Self {
        Self { iter: value.iter() }
    }
}

impl<'de, 'a> SeqAccess<'de> for ArrayAccess<'a> {
    type Error = Error;

    fn next_element_seed<K>(&mut self, seed: K) -> Result<Option<K::Value>, Error>
    where
        K: DeserializeSeed<'de>,
    {
        self.iter
            .next()
            .map(move |value| seed.deserialize(&mut Deserializer::from_value(value)))
            .transpose()
    }
}
