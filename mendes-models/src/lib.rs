use std::borrow::Cow;
use std::fmt;

pub use mendes_macros::model;

pub struct Table {
    pub name: Cow<'static, str>,
    pub columns: Vec<Column>,
    pub constraints: Vec<Constraint>,
}

impl fmt::Display for Table {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "CREATE TABLE {} (", self.name)?;
        for (i, col) in self.columns.iter().enumerate() {
            if i > 0 {
                write!(fmt, ", ")?;
            }
            write!(fmt, "{}", col)?;
        }
        for constraint in self.constraints.iter() {
            write!(fmt, ", {}", constraint)?;
        }
        write!(fmt, ")")
    }
}

pub struct Column {
    pub name: Cow<'static, str>,
    pub ty: Cow<'static, str>,
    pub null: bool,
    pub default: Option<Cow<'static, str>>,
}

impl fmt::Display for Column {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{} {}", self.name, self.ty)?;
        if !self.null {
            write!(fmt, " NOT NULL")?;
        }
        Ok(())
    }
}

pub enum Constraint {
    PrimaryKey {
        name: Cow<'static, str>,
        columns: Vec<Cow<'static, str>>,
    },
}

impl fmt::Display for Constraint {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Constraint::PrimaryKey { name, columns } => {
                write!(fmt, "CONSTRAINT {} PRIMARY KEY (", name)?;
                for (i, col) in columns.iter().enumerate() {
                    if i > 0 {
                        write!(fmt, ", ")?;
                    }
                    write!(fmt, "{}", col)?;
                }
                Ok(())
            }
        }
    }
}

pub struct Serial<T>(T);

pub trait Model<S: System> {
    fn table() -> Table;
}

pub trait ToColumn<T>: System {
    fn to_column(name: Cow<'static, str>, params: &[(&str, &str)]) -> Column;
}

impl ToColumn<Serial<i32>> for PostgreSQL {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "serial".into(),
            null: false,
            default: None,
        }
    }
}

impl ToColumn<String> for PostgreSQL {
    fn to_column(name: Cow<'static, str>, _: &[(&str, &str)]) -> Column {
        Column {
            name,
            ty: "text".into(),
            null: false,
            default: None,
        }
    }
}

pub struct PostgreSQL {}

impl System for PostgreSQL {}

pub trait System: Sized {
    fn table<M: Model<Self>>() -> Table {
        M::table()
    }
}
