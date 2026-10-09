//! Argument tokens and Rhino-style coordinate parsing.
//!
//! Supported point forms:
//! - `x,y` (z = 0) and `x,y,z` — absolute
//! - `@dx,dy` and `@dx,dy,dz` — relative to the last point
//! - `@d<a` — polar in the XY plane: distance `d`, angle `a` in degrees

use crate::CommandError;
use forma_geom::{Point3, Vec3};

pub struct Args<'a> {
    tokens: &'a [&'a str],
    pos: usize,
}

impl<'a> Args<'a> {
    pub fn new(tokens: &'a [&'a str]) -> Self {
        Self { tokens, pos: 0 }
    }

    /// Next raw token, if any.
    pub fn next_token(&mut self) -> Option<&'a str> {
        let t = self.tokens.get(self.pos).copied();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    /// Peek at the next token without consuming it.
    pub fn peek(&self) -> Option<&'a str> {
        self.tokens.get(self.pos).copied()
    }

    /// Consume the next token if it equals `keyword` (case-insensitive).
    pub fn keyword(&mut self, keyword: &str) -> bool {
        if self.peek().is_some_and(|t| t.eq_ignore_ascii_case(keyword)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Next token parsed as a point; `last` resolves `@` relative input.
    pub fn point(
        &mut self,
        what: &'static str,
        last: Option<Point3>,
    ) -> Result<Point3, CommandError> {
        let t = self.next_token().ok_or(CommandError::MissingInput(what))?;
        parse_point(t, last)
    }

    /// Next token parsed as a number.
    pub fn number(&mut self, what: &'static str) -> Result<f64, CommandError> {
        let t = self.next_token().ok_or(CommandError::MissingInput(what))?;
        t.parse()
            .map_err(|_| CommandError::BadInput(format!("{what}: {t}")))
    }

    /// Unconsumed tokens joined with spaces, or `None` if all were used.
    pub fn remaining(&self) -> Option<String> {
        (self.pos < self.tokens.len()).then(|| self.tokens[self.pos..].join(" "))
    }
}

fn num(s: &str, full: &str) -> Result<f64, CommandError> {
    s.trim()
        .parse()
        .map_err(|_| CommandError::BadInput(full.to_string()))
}

/// Parse one coordinate token. See module docs for the accepted forms.
pub fn parse_point(token: &str, last: Option<Point3>) -> Result<Point3, CommandError> {
    let (relative, body) = match token.strip_prefix('@') {
        Some(rest) => (true, rest),
        None => (false, token),
    };

    if relative {
        if let Some((d, a)) = body.split_once('<') {
            let d = num(d, token)?;
            let a = num(a, token)?.to_radians();
            let base =
                last.ok_or_else(|| CommandError::Invalid("no previous point for @".into()))?;
            return Ok(base + Vec3::new(d * a.cos(), d * a.sin(), 0.0));
        }
    }

    let parts: Vec<&str> = body.split(',').collect();
    let v = match parts.as_slice() {
        [x, y] => Vec3::new(num(x, token)?, num(y, token)?, 0.0),
        [x, y, z] => Vec3::new(num(x, token)?, num(y, token)?, num(z, token)?),
        _ => return Err(CommandError::BadInput(token.to_string())),
    };

    if relative {
        let base = last.ok_or_else(|| CommandError::Invalid("no previous point for @".into()))?;
        Ok(base + v)
    } else {
        Ok(Point3::ORIGIN + v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use forma_geom::Tolerance;

    #[test]
    fn absolute_points() {
        assert_eq!(
            parse_point("1,2", None).unwrap(),
            Point3::new(1.0, 2.0, 0.0)
        );
        assert_eq!(
            parse_point("1,2,3", None).unwrap(),
            Point3::new(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn relative_points() {
        let last = Some(Point3::new(10.0, 10.0, 0.0));
        assert_eq!(
            parse_point("@5,0", last).unwrap(),
            Point3::new(15.0, 10.0, 0.0)
        );
        let p = parse_point("@10<90", last).unwrap();
        assert!(p.almost_eq(Point3::new(10.0, 20.0, 0.0), Tolerance::default()));
    }

    #[test]
    fn relative_needs_previous_point() {
        assert!(parse_point("@1,1", None).is_err());
    }

    #[test]
    fn garbage_is_rejected() {
        assert!(parse_point("a,b", None).is_err());
        assert!(parse_point("1", None).is_err());
    }
}
