// json.rs
// Lector de JSON minimo, lo justo para leer los modelos de bloque del pack.
//
// Los modelos de Minecraft son JSON de verdad (objetos, listas, numeros,
// texto y comentarios ninguno), asi que buscar los campos a mano con
// substrings andaria hasta el dia que un modelo traiga una clave parecida.
// Son ciento y pico de lineas: sale mas barato leerlos bien.
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Json {
    Null,
    Bool(bool),
    Number(f64),
    Text(String),
    List(Vec<Json>),
    Map(HashMap<String, Json>),
}

impl Json {
    /// El valor de una clave, si esto es un objeto y la tiene.
    pub fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Json::Map(fields) => fields.get(key),
            _ => None,
        }
    }

    pub fn list(&self) -> Option<&[Json]> {
        match self {
            Json::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn number(&self) -> Option<f64> {
        match self {
            Json::Number(value) => Some(*value),
            _ => None,
        }
    }

    /// Lee un documento entero. Sobra lo que venga despues del primer valor:
    /// un archivo con basura al final no es JSON valido.
    pub fn parse(source: &str) -> Result<Json, String> {
        let mut reader = Reader {
            bytes: source.as_bytes(),
            at: 0,
        };

        let value = reader.value()?;
        reader.spaces();
        if reader.at < reader.bytes.len() {
            return Err(format!("sobra texto en el byte {}", reader.at));
        }

        Ok(value)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn spaces(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            self.at += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    /// Se come el byte que viene si es el esperado.
    fn eat(&mut self, byte: u8) -> bool {
        if self.peek() == Some(byte) {
            self.at += 1;
            return true;
        }
        false
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.eat(byte) {
            return Ok(());
        }
        Err(format!(
            "esperaba '{}' en el byte {}",
            byte as char, self.at
        ))
    }

    fn value(&mut self) -> Result<Json, String> {
        self.spaces();

        match self.peek() {
            Some(b'{') => self.map(),
            Some(b'[') => self.list(),
            Some(b'"') => Ok(Json::Text(self.text()?)),
            Some(b't') => self.word("true", Json::Bool(true)),
            Some(b'f') => self.word("false", Json::Bool(false)),
            Some(b'n') => self.word("null", Json::Null),
            Some(_) => self.number(),
            None => Err("el documento se corto".to_string()),
        }
    }

    fn word(&mut self, word: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            return Ok(value);
        }
        Err(format!("palabra desconocida en el byte {}", self.at))
    }

    fn map(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut fields = HashMap::new();

        self.spaces();
        if self.eat(b'}') {
            return Ok(Json::Map(fields));
        }

        loop {
            self.spaces();
            let key = self.text()?;
            self.spaces();
            self.expect(b':')?;
            fields.insert(key, self.value()?);

            self.spaces();
            if self.eat(b',') {
                continue;
            }
            self.expect(b'}')?;
            return Ok(Json::Map(fields));
        }
    }

    fn list(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();

        self.spaces();
        if self.eat(b']') {
            return Ok(Json::List(items));
        }

        loop {
            items.push(self.value()?);

            self.spaces();
            if self.eat(b',') {
                continue;
            }
            self.expect(b']')?;
            return Ok(Json::List(items));
        }
    }

    fn text(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();

        loop {
            let byte = self.peek().ok_or("el texto se corto")?;
            self.at += 1;

            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = self.peek().ok_or("el escape se corto")?;
                    self.at += 1;
                    out.push(match escape {
                        b'n' => '\n',
                        b't' => '\t',
                        b'r' => '\r',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        // \uXXXX no aparece en los modelos: se deja el crudo
                        other => other as char,
                    });
                }
                other => out.push(other as char),
            }
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.at;
        while matches!(
            self.peek(),
            Some(b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
        ) {
            self.at += 1;
        }

        std::str::from_utf8(&self.bytes[start..self.at])
            .ok()
            .and_then(|text| text.parse().ok())
            .map(Json::Number)
            .ok_or_else(|| format!("numero invalido en el byte {start}"))
    }
}
