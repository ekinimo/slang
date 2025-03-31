#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunIdx(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeIdx(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TypeVarIdx(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AstIdx(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NameIdx(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParamIdx(pub usize);

impl From<usize> for TypeIdx {
    fn from(idx: usize) -> Self {
        TypeIdx(idx)
    }
}

impl From<TypeIdx> for usize {
    fn from(idx: TypeIdx) -> Self {
        idx.0
    }
}

impl From<usize> for TypeVarIdx {
    fn from(idx: usize) -> Self {
        TypeVarIdx(idx)
    }
}

impl From<TypeVarIdx> for usize {
    fn from(idx: TypeVarIdx) -> Self {
        idx.0
    }
}

impl From<usize> for FunIdx {
    fn from(idx: usize) -> Self {
        FunIdx(idx)
    }
}

impl From<FunIdx> for usize {
    fn from(idx: FunIdx) -> Self {
        idx.0
    }
}

impl From<usize> for AstIdx {
    fn from(idx: usize) -> Self {
        AstIdx(idx)
    }
}

impl From<AstIdx> for usize {
    fn from(idx: AstIdx) -> Self {
        idx.0
    }
}

impl From<usize> for NameIdx {
    fn from(idx: usize) -> Self {
        NameIdx(idx)
    }
}

impl From<NameIdx> for usize {
    fn from(idx: NameIdx) -> Self {
        idx.0
    }
}

impl From<usize> for ParamIdx {
    fn from(idx: usize) -> Self {
        ParamIdx(idx)
    }
}

impl From<ParamIdx> for usize {
    fn from(idx: ParamIdx) -> Self {
        idx.0
    }
}
