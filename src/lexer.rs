// Celefun 词法分析器
// 把 .celefun 源码切成 token；箭头按最长匹配；注释是 ↑↑↑ ... ↓↓↓（可跨行）。

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    True,
    False,
    // 关键字
    Determ,  // 决心
    Door,    // 心门
    NotOpen, // 没开
    Loop,    // 凌波
    Brk,     // 狼跳
    Cont,    // 奥欻
    Exit,    // 心
    Goto,    // 金羽毛
    Label,   // Badeline
    Declare, // 旗帜
    Ret,     // 超级弹
    Panic,   // 踩刺
    TInt,    // 草莓
    TFloat,  // 金草莓
    TBool,   // 磁带
    TJelly,  // 果冻块
    Pico8,   // PICO-8（标记要暴露的 C FFI 面）
    // 符号
    LBrace, // →↘
    RBrace, // ↘→
    Assign, // →←
    Eq,     // ←→
    Ne,     // ↑↓ 或 ↓↑
    Le,     // ←←
    Ge,     // →→
    Lt,     // ←
    Gt,     // →
    And,    // ↘↘
    Or,     // ↙↙
    Not,    // ↖↖
    Neg,    // ↖
    Add,    // ↑
    Sub,    // ↓
    Mul,    // ↗
    Div,    // ↘
    Mod,    // ↙
    Inc,    // ↑↑
    Dec,    // ↓↓
    Semi,   // ;
    Colon,  // :
    Sep,    // 、 或 ,
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    pub col: u32,
}

#[derive(Debug, Clone)]
pub struct LexError {
    pub msg: String,
    pub line: u32,
    pub col: u32,
}

pub fn tokenize(src: &str) -> Result<Vec<Token>, LexError> {
    Lexer::new(src).run()
}

struct Lexer {
    chars: Vec<char>,
    pos: usize,
    line: u32,
    col: u32,
}

const TWO: &[(&str, Tok)] = &[
    ("→↘", Tok::LBrace),
    ("↘→", Tok::RBrace),
    ("→←", Tok::Assign),
    ("←→", Tok::Eq),
    ("↑↓", Tok::Ne),
    ("↓↑", Tok::Ne),
    ("←←", Tok::Le),
    ("→→", Tok::Ge),
    ("↘↘", Tok::And),
    ("↙↙", Tok::Or),
    ("↖↖", Tok::Not),
    ("↑↑", Tok::Inc),
    ("↓↓", Tok::Dec),
];

impl Lexer {
    fn new(src: &str) -> Self {
        Lexer { chars: src.chars().collect(), pos: 0, line: 1, col: 1 }
    }
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }
    fn starts_with(&self, s: &str) -> bool {
        let mut i = self.pos;
        for c in s.chars() {
            if self.chars.get(i) != Some(&c) {
                return false;
            }
            i += 1;
        }
        true
    }
    fn bump(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.pos += 1;
        if c == '\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        Some(c)
    }
    fn eat_str(&mut self, s: &str) {
        for _ in s.chars() {
            self.bump();
        }
    }
    fn err(&self, msg: impl Into<String>) -> LexError {
        LexError { msg: msg.into(), line: self.line, col: self.col }
    }

    fn skip_trivia(&mut self) -> Result<(), LexError> {
        loop {
            match self.peek() {
                Some(c) if c.is_whitespace() => {
                    self.bump();
                }
                _ if self.starts_with("↑↑↑") => {
                    let (l, c) = (self.line, self.col);
                    self.eat_str("↑↑↑");
                    let mut closed = false;
                    while self.pos < self.chars.len() {
                        if self.starts_with("↓↓↓") {
                            self.eat_str("↓↓↓");
                            closed = true;
                            break;
                        }
                        self.bump();
                    }
                    if !closed {
                        return Err(LexError { msg: "注释没有用 ↓↓↓ 收尾".into(), line: l, col: c });
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn scan_token(&mut self) -> Result<Tok, LexError> {
        if self.starts_with("PICO-8") {
            self.eat_str("PICO-8");
            return Ok(Tok::Pico8);
        }
        for (s, t) in TWO {
            if self.starts_with(s) {
                let t = t.clone();
                self.eat_str(s);
                return Ok(t);
            }
        }
        let ch = self.peek().unwrap();
        let one = match ch {
            '↑' => Tok::Add,
            '↓' => Tok::Sub,
            '↗' => Tok::Mul,
            '↘' => Tok::Div,
            '↙' => Tok::Mod,
            '↖' => Tok::Neg,
            '←' => Tok::Lt,
            '→' => Tok::Gt,
            ';' => Tok::Semi,
            ':' => Tok::Colon,
            '、' | ',' => Tok::Sep,
            _ => {
                if ch.is_ascii_digit() {
                    return self.scan_number();
                }
                if is_ident_start(ch) {
                    return Ok(self.scan_ident());
                }
                return Err(self.err(format!("不认识的字符：{}", ch)));
            }
        };
        self.bump();
        Ok(one)
    }

    fn scan_number(&mut self) -> Result<Tok, LexError> {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        if self.peek() == Some('.') {
            let next = self.chars.get(self.pos + 1).copied();
            if matches!(next, Some(c) if c.is_ascii_digit()) {
                s.push('.');
                self.bump();
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                let v: f64 = s.parse().map_err(|_| self.err("小数格式不对"))?;
                return Ok(Tok::Float(v));
            }
        }
        let v: i64 = s.parse().map_err(|_| self.err("整数太大"))?;
        Ok(Tok::Int(v))
    }

    fn scan_ident(&mut self) -> Tok {
        let mut s = String::new();
        while let Some(c) = self.peek() {
            if is_ident_char(c) {
                s.push(c);
                self.bump();
            } else {
                break;
            }
        }
        match s.as_str() {
            "决心" => Tok::Determ,
            "心门" => Tok::Door,
            "没开" => Tok::NotOpen,
            "凌波" => Tok::Loop,
            "狼跳" => Tok::Brk,
            "奥欻" => Tok::Cont,
            "心" => Tok::Exit,
            "金羽毛" => Tok::Goto,
            "Badeline" => Tok::Label,
            "旗帜" => Tok::Declare,
            "超级弹" => Tok::Ret,
            "踩刺" => Tok::Panic,
            "草莓" => Tok::TInt,
            "金草莓" => Tok::TFloat,
            "磁带" => Tok::TBool,
            "果冻块" => Tok::TJelly,
            "true" => Tok::True,
            "false" => Tok::False,
            _ => Tok::Ident(s),
        }
    }

    fn run(mut self) -> Result<Vec<Token>, LexError> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia()?;
            if self.pos >= self.chars.len() {
                out.push(Token { tok: Tok::Eof, line: self.line, col: self.col });
                return Ok(out);
            }
            let (line, col) = (self.line, self.col);
            let tok = self.scan_token()?;
            out.push(Token { tok, line, col });
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        tokenize(src).unwrap().into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn arrows_longest_match() {
        let t = toks("→↘ ↘→ →← ←→ ↑↓ ↘↘ ↑↑ ↓↓");
        assert_eq!(t[0], Tok::LBrace);
        assert_eq!(t[1], Tok::RBrace);
        assert_eq!(t[2], Tok::Assign);
        assert_eq!(t[3], Tok::Eq);
        assert_eq!(t[4], Tok::Ne);
        assert_eq!(t[5], Tok::And);
        assert_eq!(t[6], Tok::Inc);
        assert_eq!(t[7], Tok::Dec);
    }

    #[test]
    fn keywords_and_types() {
        let src = "决心 心门 没开 凌波 狼跳 奥欻 心 旗帜 超级弹 踩刺 金羽毛 Badeline 草莓 金草莓 磁带 果冻块 true false";
        let t = toks(src);
        let want = [
            Tok::Determ, Tok::Door, Tok::NotOpen, Tok::Loop, Tok::Brk, Tok::Cont,
            Tok::Exit, Tok::Declare, Tok::Ret, Tok::Panic, Tok::Goto, Tok::Label,
            Tok::TInt, Tok::TFloat, Tok::TBool, Tok::TJelly, Tok::True, Tok::False,
        ];
        for (i, w) in want.iter().enumerate() {
            assert_eq!(&t[i], w);
        }
    }

    #[test]
    fn comments() {
        let t = toks("↑↑↑ 随便写 ↑↓ ↑↑ ↓↓↓ 1");
        assert_eq!(t[0], Tok::Int(1));
    }

    #[test]
    fn numbers_and_idents() {
        let t = toks("x1 42 3.5 第九章第一面");
        assert_eq!(t[0], Tok::Ident("x1".into()));
        assert_eq!(t[1], Tok::Int(42));
        assert_eq!(t[2], Tok::Float(3.5));
        assert_eq!(t[3], Tok::Ident("第九章第一面".into()));
    }
}

