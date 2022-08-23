use std::collections::HashMap;
use std::hash::Hash;

use glsl::syntax::{ArraySpecifier, ArraySpecifierDimension, BinaryOp, Declaration, Expr, ExternalDeclaration, FunIdentifier, Identifier, Initializer, NonEmpty, ShaderStage, StructSpecifier, TranslationUnit, TypeSpecifier, TypeSpecifierNonArray, UnaryOp};
use glsl::visitor::{Visit, VisitorMut};
use nalgebra::{Matrix2, Matrix2x3, Matrix2x4, Matrix3, Matrix3x2, Matrix3x4, Matrix4, Matrix4x2, Matrix4x3, Scalar, Vector2, Vector3, Vector4};

use paste::paste;

pub use function::{ConstEvalFunctionBuilder, ConstEvalFunction};

/// Utility macro for from/try_from impls on ConstXVal types. Avoids a few thousand lines of code
macro_rules! impl_from_to_const_val {
    ($target:ident, $from:ident, $variant:ident) => {
        impl<T: Scalar> From<$from <T>> for $target <T> {
            fn from(v: $from <T>) -> Self {
                $target::$variant(v.into())
            }
        }
        impl<T: Scalar> TryFrom<$target <T>> for $from <T> {
            type Error = ();

            fn try_from(value: $target <T>) -> Result<Self, Self::Error> {
                match value {
                    $target::$variant(v) => v.try_into().ok().ok_or(()),
                    _ => Err(()),
                }
            }
        }
    };
}

/// Utility macro for try_from impls on explicit typed ConstXVal types. Avoids a few thousand lines of code
macro_rules! impl_try_from_const_val_explicit {
    ($target:ident, $gen:ty, $from:ty, $variant:ident) => {
        impl TryFrom<$target <$gen>> for $from {
            type Error = ();

            fn try_from(value: $target <$gen>) -> Result<Self, Self::Error> {
                match value {
                    $target::$variant(v) => v.try_into().ok().ok_or(()),
                    _ => Err(()),
                }
            }
        }
    };
}

/// Allows lookup of constant values
pub trait ConstLookup {
    fn lookup_const(&self, ident: &Identifier) -> Option<&ConstVal>;

    fn is_const(&self, ident: &Identifier) -> bool {
        self.lookup_const(ident).is_some()
    }
}

/// Allows lookup of const evaluable functions
pub trait ConstEvalFunctionLookup {
    fn lookup(&self, ident: &Identifier) -> Option<&ConstEvalFunction>;
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum BaseTypeShape {
    Scalar,
    Vec2,
    Vec3,
    Vec4,
    Mat2,
    Mat23,
    Mat24,
    Mat32,
    Mat3,
    Mat34,
    Mat42,
    Mat43,
    Mat4
}

impl BaseTypeShape {
    fn is_scalar(&self) -> bool {
        match self {
            BaseTypeShape::Scalar => true,
            _ => false,
        }
    }

    fn is_vector(&self) -> bool {
        match self {
            BaseTypeShape::Vec2 |
            BaseTypeShape::Vec3 |
            BaseTypeShape::Vec4 => true,
            _ => false,
        }
    }

    fn is_matrix(&self) -> bool {
        match self {
            BaseTypeShape::Mat2 |
            BaseTypeShape::Mat23 |
            BaseTypeShape::Mat24 |
            BaseTypeShape::Mat32 |
            BaseTypeShape::Mat3 |
            BaseTypeShape::Mat34 |
            BaseTypeShape::Mat42 |
            BaseTypeShape::Mat43 |
            BaseTypeShape::Mat4 => true,
            _ => false,
        }
    }
}

/// General utility functions to work with generic shaped constant values
pub trait ConstGenericValue<'a, T: Scalar> {
    fn get_shape(&'a self) -> BaseTypeShape;

    fn is_scalar(&'a self) -> bool {
        self.get_shape().is_scalar()
    }

    fn is_vector(&'a self) -> bool {
        self.get_shape().is_vector()
    }

    fn is_matrix(&'a self) -> bool {
        self.get_shape().is_matrix()
    }

    type ColumnIterator: Iterator<Item = &'a T>;

    /// Iterates over all elements in column major order.
    fn column_iter(&'a self) -> Self::ColumnIterator;

    fn fold<R, F: FnMut(R, T) -> R>(&'a self, initial: R, f: F) -> R {
        self.column_iter().cloned().fold(initial, f)
    }
}

/// Functions for component wise mapping of a generic shaped constant value
pub trait ConstGenericMappable<'a, 'b, T: Scalar, R: Scalar> {
    type Result: ConstGenericValue<'b, R>;

    /// Applies the function to each component returning a new ConstGenericValue of the same shape.
    fn map<F: FnMut(T) -> R>(&'a self, f: F) -> Self::Result;
}

/// Functions for component wise mapping of 2 generic shaped constant values
pub trait ConstGenericZipMappable<'a, 'b, 'c, T1: Scalar, T2: Scalar, O: ConstGenericValue<'b, T2>, R: Scalar> {
    type Result: ConstGenericValue<'c, R>;

    /// Applies the function to each component pair returning a new ConstGenericValue of the same
    /// shape.
    ///
    /// If `other` does not have the same shape as `self`, [`None`] must be returned.
    fn zip_map<F: FnMut(T1, T2) -> R>(&'a self, other: &'b O, f: F) -> Option<Self::Result>;
}

/// Constant generic shaped vector value.
#[derive(Clone, PartialEq, Hash, Debug)]
pub enum ConstVVal<T: Scalar> {
    Vec2(Vector2<T>),
    Vec3(Vector3<T>),
    Vec4(Vector4<T>),
}

impl<T: Scalar> ConstVVal<T> {
    pub fn new_vec2<S: Into<Vector2<T>>>(val: S) -> Self {
        Self::Vec2(val.into())
    }

    pub fn new_vec3<S: Into<Vector3<T>>>(val: S) -> Self {
        Self::Vec3(val.into())
    }

    pub fn new_vec4<S: Into<Vector4<T>>>(val: S) -> Self {
        Self::Vec4(val.into())
    }
}

impl<'a, T: Scalar> ConstGenericValue<'a, T> for ConstVVal<T> {
    fn get_shape(&'a self) -> BaseTypeShape {
        match self {
            ConstVVal::Vec2(_) => BaseTypeShape::Vec2,
            ConstVVal::Vec3(_) => BaseTypeShape::Vec3,
            ConstVVal::Vec4(_) => BaseTypeShape::Vec4,
        }
    }

    type ColumnIterator = std::slice::Iter<'a, T>;

    fn column_iter(&'a self) -> Self::ColumnIterator {
        match self {
            ConstVVal::Vec2(v) => v.as_slice().iter(),
            ConstVVal::Vec3(v) => v.as_slice().iter(),
            ConstVVal::Vec4(v) => v.as_slice().iter(),
        }
    }
}

impl<'a, 'b, T: Scalar, R: Scalar> ConstGenericMappable<'a, 'b, T, R> for ConstVVal<T> {
    type Result = ConstVVal<R>;

    fn map<F: FnMut(T) -> R>(&'a self, f: F) -> Self::Result {
        match self {
            ConstVVal::Vec2(v) => ConstVVal::Vec2(v.map(f)),
            ConstVVal::Vec3(v) => ConstVVal::Vec3(v.map(f)),
            ConstVVal::Vec4(v) => ConstVVal::Vec4(v.map(f)),
        }
    }
}

impl<'a, 'b, 'c, T1: Scalar, T2: Scalar, R: Scalar> ConstGenericZipMappable<'a, 'b, 'c, T1, T2, ConstVVal<T2>, R> for ConstVVal<T1> {
    type Result = ConstVVal<R>;

    fn zip_map<F: FnMut(T1, T2) -> R>(&'a self, other: &'b ConstVVal<T2>, f: F) -> Option<Self::Result> {
        match (self, other) {
            (ConstVVal::Vec2(v1), ConstVVal::Vec2(v2)) => Some(ConstVVal::Vec2(v1.zip_map(v2, f))),
            (ConstVVal::Vec3(v1), ConstVVal::Vec3(v2)) => Some(ConstVVal::Vec3(v1.zip_map(v2, f))),
            (ConstVVal::Vec4(v1), ConstVVal::Vec4(v2)) => Some(ConstVVal::Vec4(v1.zip_map(v2, f))),
            _ => None,
        }
    }
}

impl_from_to_const_val!(ConstVVal, Vector2, Vec2);
impl_from_to_const_val!(ConstVVal, Vector3, Vec3);
impl_from_to_const_val!(ConstVVal, Vector4, Vec4);

/// Constant generic shaped matrix value.
#[derive(Clone, PartialEq, Hash, Debug)]
pub enum ConstMVal<T: Scalar> {
    Mat2(Matrix2<T>),
    Mat23(Matrix2x3<T>),
    Mat24(Matrix2x4<T>),
    Mat32(Matrix3x2<T>),
    Mat3(Matrix3<T>),
    Mat34(Matrix3x4<T>),
    Mat42(Matrix4x2<T>),
    Mat43(Matrix4x3<T>),
    Mat4(Matrix4<T>),
}

impl<T: Scalar> ConstMVal<T> {
    pub fn new_mat2<S: Into<Matrix2<T>>>(val: S) -> Self {
        Self::Mat2(val.into())
    }

    pub fn new_mat23<S: Into<Matrix2x3<T>>>(val: S) -> Self {
        Self::Mat23(val.into())
    }

    pub fn new_mat24<S: Into<Matrix2x4<T>>>(val: S) -> Self {
        Self::Mat24(val.into())
    }

    pub fn new_mat32<S: Into<Matrix3x2<T>>>(val: S) -> Self {
        Self::Mat32(val.into())
    }

    pub fn new_mat3<S: Into<Matrix3<T>>>(val: S) -> Self {
        Self::Mat3(val.into())
    }

    pub fn new_mat34<S: Into<Matrix3x4<T>>>(val: S) -> Self {
        Self::Mat34(val.into())
    }

    pub fn new_mat42<S: Into<Matrix4x2<T>>>(val: S) -> Self {
        Self::Mat42(val.into())
    }

    pub fn new_mat43<S: Into<Matrix4x3<T>>>(val: S) -> Self {
        Self::Mat43(val.into())
    }

    pub fn new_mat4<S: Into<Matrix4<T>>>(val: S) -> Self {
        Self::Mat4(val.into())
    }
}

impl<'a, T: Scalar> ConstGenericValue<'a, T> for ConstMVal<T> {
    fn get_shape(&'a self) -> BaseTypeShape {
        match self {
            ConstMVal::Mat2(_) => BaseTypeShape::Mat2,
            ConstMVal::Mat23(_) => BaseTypeShape::Mat23,
            ConstMVal::Mat24(_) => BaseTypeShape::Mat24,
            ConstMVal::Mat32(_) => BaseTypeShape::Mat32,
            ConstMVal::Mat3(_) => BaseTypeShape::Mat3,
            ConstMVal::Mat34(_) => BaseTypeShape::Mat34,
            ConstMVal::Mat42(_) => BaseTypeShape::Mat42,
            ConstMVal::Mat43(_) => BaseTypeShape::Mat43,
            ConstMVal::Mat4(_) => BaseTypeShape::Mat4,
        }
    }

    type ColumnIterator = std::slice::Iter<'a, T>;

    fn column_iter(&'a self) -> Self::ColumnIterator {
        match self {
            ConstMVal::Mat2(v) => v.as_slice().iter(),
            ConstMVal::Mat23(v) => v.as_slice().iter(),
            ConstMVal::Mat24(v) => v.as_slice().iter(),
            ConstMVal::Mat32(v) => v.as_slice().iter(),
            ConstMVal::Mat3(v) => v.as_slice().iter(),
            ConstMVal::Mat34(v) => v.as_slice().iter(),
            ConstMVal::Mat42(v) => v.as_slice().iter(),
            ConstMVal::Mat43(v) => v.as_slice().iter(),
            ConstMVal::Mat4(v) => v.as_slice().iter(),
        }
    }
}

impl<'a, 'b, T: Scalar, R: Scalar> ConstGenericMappable<'a, 'b, T, R> for ConstMVal<T> {
    type Result = ConstMVal<R>;

    fn map<F: FnMut(T) -> R>(&'a self, f: F) -> Self::Result {
        match self {
            ConstMVal::Mat2(v) => ConstMVal::Mat2(v.map(f)),
            ConstMVal::Mat23(v) => ConstMVal::Mat23(v.map(f)),
            ConstMVal::Mat24(v) => ConstMVal::Mat24(v.map(f)),
            ConstMVal::Mat32(v) => ConstMVal::Mat32(v.map(f)),
            ConstMVal::Mat3(v) => ConstMVal::Mat3(v.map(f)),
            ConstMVal::Mat34(v) => ConstMVal::Mat34(v.map(f)),
            ConstMVal::Mat42(v) => ConstMVal::Mat42(v.map(f)),
            ConstMVal::Mat43(v) => ConstMVal::Mat43(v.map(f)),
            ConstMVal::Mat4(v) => ConstMVal::Mat4(v.map(f)),
        }
    }
}

impl<'a, 'b, 'c, T1: Scalar, T2: Scalar, R: Scalar> ConstGenericZipMappable<'a, 'b, 'c, T1, T2, ConstMVal<T2>, R> for ConstMVal<T1> {
    type Result = ConstMVal<R>;

    fn zip_map<F: FnMut(T1, T2) -> R>(&'a self, other: &'b ConstMVal<T2>, f: F) -> Option<Self::Result> {
        match (self, other) {
            (ConstMVal::Mat2(a), ConstMVal::Mat2(b)) => Some(ConstMVal::Mat2(a.zip_map(b, f))),
            (ConstMVal::Mat23(a), ConstMVal::Mat23(b)) => Some(ConstMVal::Mat23(a.zip_map(b, f))),
            (ConstMVal::Mat24(a), ConstMVal::Mat24(b)) => Some(ConstMVal::Mat24(a.zip_map(b, f))),
            (ConstMVal::Mat32(a), ConstMVal::Mat32(b)) => Some(ConstMVal::Mat32(a.zip_map(b, f))),
            (ConstMVal::Mat3(a), ConstMVal::Mat3(b)) => Some(ConstMVal::Mat3(a.zip_map(b, f))),
            (ConstMVal::Mat34(a), ConstMVal::Mat34(b)) => Some(ConstMVal::Mat34(a.zip_map(b, f))),
            (ConstMVal::Mat42(a), ConstMVal::Mat42(b)) => Some(ConstMVal::Mat42(a.zip_map(b, f))),
            (ConstMVal::Mat43(a), ConstMVal::Mat43(b)) => Some(ConstMVal::Mat43(a.zip_map(b, f))),
            (ConstMVal::Mat4(a), ConstMVal::Mat4(b)) => Some(ConstMVal::Mat4(a.zip_map(b, f))),
            _ => None
        }
    }
}

impl_from_to_const_val!(ConstMVal, Matrix2, Mat2);
impl_from_to_const_val!(ConstMVal, Matrix2x3, Mat23);
impl_from_to_const_val!(ConstMVal, Matrix2x4, Mat24);
impl_from_to_const_val!(ConstMVal, Matrix3x2, Mat32);
impl_from_to_const_val!(ConstMVal, Matrix3, Mat3);
impl_from_to_const_val!(ConstMVal, Matrix3x4, Mat34);
impl_from_to_const_val!(ConstMVal, Matrix4x2, Mat42);
impl_from_to_const_val!(ConstMVal, Matrix4x3, Mat43);
impl_from_to_const_val!(ConstMVal, Matrix4, Mat4);

/// Constant generic shaped scalar or vector value
#[derive(Clone, PartialEq, Hash, Debug)]
pub enum ConstSVVal<T: Scalar> {
    Scalar(T),
    Vector(ConstVVal<T>),
}

impl<T: Scalar> ConstSVVal<T> {
    pub fn new_scalar<S: Into<T>>(val: S) -> Self {
        Self::Scalar(val.into())
    }

    pub fn new_vec2<S: Into<Vector2<T>>>(val: S) -> Self {
        Self::Vector(ConstVVal::Vec2(val.into()))
    }

    pub fn new_vec3<S: Into<Vector3<T>>>(val: S) -> Self {
        Self::Vector(ConstVVal::Vec3(val.into()))
    }

    pub fn new_vec4<S: Into<Vector4<T>>>(val: S) -> Self {
        Self::Vector(ConstVVal::Vec4(val.into()))
    }
}

impl<'a, T: Scalar> ConstGenericValue<'a, T> for ConstSVVal<T> {
    fn get_shape(&'a self) -> BaseTypeShape {
        match self {
            ConstSVVal::Scalar(_) => BaseTypeShape::Scalar,
            ConstSVVal::Vector(v) => v.get_shape(),
        }
    }

    type ColumnIterator = std::slice::Iter<'a, T>;

    fn column_iter(&'a self) -> Self::ColumnIterator {
        match self {
            ConstSVVal::Scalar(v) => std::slice::from_ref(v).iter(),
            ConstSVVal::Vector(v) => v.column_iter(),
        }
    }
}

impl<'a, 'b, T: Scalar, R: Scalar> ConstGenericMappable<'a, 'b, T, R> for ConstSVVal<T> {
    type Result = ConstSVVal<R>;

    fn map<F: FnMut(T) -> R>(&'a self, mut f: F) -> Self::Result {
        match self {
            ConstSVVal::Scalar(v) => ConstSVVal::Scalar(f(v.clone())),
            ConstSVVal::Vector(v) => ConstSVVal::Vector(v.map(f)),
        }
    }
}

impl<'a, 'b, 'c, T1: Scalar, T2: Scalar, R: Scalar> ConstGenericZipMappable<'a, 'b, 'c, T1, T2, ConstSVVal<T2>, R> for ConstSVVal<T1> {
    type Result = ConstSVVal<R>;

    fn zip_map<F: FnMut(T1, T2) -> R>(&'a self, other: &'b ConstSVVal<T2>, mut f: F) -> Option<Self::Result> {
        match (self, other) {
            (ConstSVVal::Scalar(v1), ConstSVVal::Scalar(v2)) => Some(ConstSVVal::Scalar(f(v1.clone(), v2.clone()))),
            (ConstSVVal::Vector(v1), ConstSVVal::Vector(v2)) => v1.zip_map(v2, f).map(ConstSVVal::Vector),
            _ => None,
        }
    }
}

impl<T: Scalar> From<T> for ConstSVVal<T> {
    fn from(v: T) -> Self {
        ConstSVVal::Scalar(v)
    }
}

impl_from_to_const_val!(ConstSVVal, Vector2, Vector);
impl_from_to_const_val!(ConstSVVal, Vector3, Vector);
impl_from_to_const_val!(ConstSVVal, Vector4, Vector);
impl_from_to_const_val!(ConstSVVal, ConstVVal, Vector);

impl_try_from_const_val_explicit!(ConstSVVal, bool, bool, Scalar);
impl_try_from_const_val_explicit!(ConstSVVal, i32, i32, Scalar);
impl_try_from_const_val_explicit!(ConstSVVal, u32, u32, Scalar);
impl_try_from_const_val_explicit!(ConstSVVal, f32, f32, Scalar);
impl_try_from_const_val_explicit!(ConstSVVal, f64, f64, Scalar);

/// Constant generic shaped scalar, vector or matrix value
#[derive(Clone, PartialEq, Hash, Debug)]
pub enum ConstSVMVal<T: Scalar> {
    Scalar(T),
    Vector(ConstVVal<T>),
    Matrix(ConstMVal<T>),
}

impl<T: Scalar> ConstSVMVal<T> {
    pub fn new_scalar<S: Into<T>>(val: S) -> Self {
        Self::Scalar(val.into())
    }

    pub fn new_vec2<S: Into<Vector2<T>>>(val: S) -> Self {
        Self::Vector(ConstVVal::Vec2(val.into()))
    }

    pub fn new_vec3<S: Into<Vector3<T>>>(val: S) -> Self {
        Self::Vector(ConstVVal::Vec3(val.into()))
    }

    pub fn new_vec4<S: Into<Vector4<T>>>(val: S) -> Self {
        Self::Vector(ConstVVal::Vec4(val.into()))
    }

    pub fn new_mat2<S: Into<Matrix2<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat2(val.into()))
    }

    pub fn new_mat23<S: Into<Matrix2x3<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat23(val.into()))
    }

    pub fn new_mat24<S: Into<Matrix2x4<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat24(val.into()))
    }

    pub fn new_mat32<S: Into<Matrix3x2<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat32(val.into()))
    }

    pub fn new_mat3<S: Into<Matrix3<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat3(val.into()))
    }

    pub fn new_mat34<S: Into<Matrix3x4<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat34(val.into()))
    }

    pub fn new_mat42<S: Into<Matrix4x2<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat42(val.into()))
    }

    pub fn new_mat43<S: Into<Matrix4x3<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat43(val.into()))
    }

    pub fn new_mat4<S: Into<Matrix4<T>>>(val: S) -> Self {
        Self::Matrix(ConstMVal::Mat4(val.into()))
    }
}

impl<'a, T: Scalar> ConstGenericValue<'a, T> for ConstSVMVal<T> {
    fn get_shape(&'a self) -> BaseTypeShape {
        match self {
            ConstSVMVal::Scalar(_) => BaseTypeShape::Scalar,
            ConstSVMVal::Vector(v) => v.get_shape(),
            ConstSVMVal::Matrix(v) => v.get_shape(),
        }
    }

    type ColumnIterator = std::slice::Iter<'a, T>;

    fn column_iter(&'a self) -> Self::ColumnIterator {
        match self {
            ConstSVMVal::Scalar(v) => std::slice::from_ref(v).iter(),
            ConstSVMVal::Vector(v) => v.column_iter(),
            ConstSVMVal::Matrix(v) => v.column_iter(),
        }
    }
}

impl<'a, 'b, T: Scalar, R: Scalar> ConstGenericMappable<'a, 'b, T, R> for ConstSVMVal<T> {
    type Result = ConstSVMVal<R>;

    fn map<F: FnMut(T) -> R>(&'a self, mut f: F) -> Self::Result {
        match self {
            ConstSVMVal::Scalar(v) => ConstSVMVal::Scalar(f(v.clone())),
            ConstSVMVal::Vector(v) => ConstSVMVal::Vector(v.map(f)),
            ConstSVMVal::Matrix(v) => ConstSVMVal::Matrix(v.map(f)),
        }
    }
}

impl<'a, 'b, 'c, T1: Scalar, T2: Scalar, R: Scalar> ConstGenericZipMappable<'a, 'b, 'c, T1, T2, ConstSVMVal<T2>, R> for ConstSVMVal<T1> {
    type Result = ConstSVMVal<R>;

    fn zip_map<F: FnMut(T1, T2) -> R>(&'a self, other: &'b ConstSVMVal<T2>, mut f: F) -> Option<Self::Result> {
        match (self, other) {
            (ConstSVMVal::Scalar(v1), ConstSVMVal::Scalar(v2)) => Some(ConstSVMVal::Scalar(f(v1.clone(), v2.clone()))),
            (ConstSVMVal::Vector(v1), ConstSVMVal::Vector(v2)) => v1.zip_map(v2, f).map(ConstSVMVal::Vector),
            (ConstSVMVal::Matrix(v1), ConstSVMVal::Matrix(v2)) => v1.zip_map(v2, f).map(ConstSVMVal::Matrix),
            _ => None,
        }
    }
}

impl<T: Scalar> From<T> for ConstSVMVal<T> {
    fn from(v: T) -> Self {
        ConstSVMVal::Scalar(v)
    }
}

impl_from_to_const_val!(ConstSVMVal, Vector2, Vector);
impl_from_to_const_val!(ConstSVMVal, Vector3, Vector);
impl_from_to_const_val!(ConstSVMVal, Vector4, Vector);
impl_from_to_const_val!(ConstSVMVal, Matrix2, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix2x3, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix2x4, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix3x2, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix3, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix3x4, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix4x2, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix4x3, Matrix);
impl_from_to_const_val!(ConstSVMVal, Matrix4, Matrix);
impl_from_to_const_val!(ConstSVMVal, ConstVVal, Vector);
impl_from_to_const_val!(ConstSVMVal, ConstMVal, Matrix);

impl<T: Scalar> From<ConstSVVal<T>> for ConstSVMVal<T> {
    fn from(v: ConstSVVal<T>) -> Self {
        match v {
            ConstSVVal::Scalar(v) => ConstSVMVal::Scalar(v),
            ConstSVVal::Vector(v) => ConstSVMVal::Vector(v),
        }
    }
}

impl<T: Scalar> TryFrom<ConstSVMVal<T>> for ConstSVVal<T> {
    type Error = ();

    fn try_from(value: ConstSVMVal<T>) -> Result<Self, Self::Error> {
        match value {
            ConstSVMVal::Scalar(v) => Ok(ConstSVVal::Scalar(v)),
            ConstSVMVal::Vector(v) => Ok(ConstSVVal::Vector(v)),
            _ => Err(()),
        }
    }
}

impl_try_from_const_val_explicit!(ConstSVMVal, bool, bool, Scalar);
impl_try_from_const_val_explicit!(ConstSVMVal, i32, i32, Scalar);
impl_try_from_const_val_explicit!(ConstSVMVal, u32, u32, Scalar);
impl_try_from_const_val_explicit!(ConstSVMVal, f32, f32, Scalar);
impl_try_from_const_val_explicit!(ConstSVMVal, f64, f64, Scalar);

macro_rules! impl_const_base_val_internal {
    ($variant:ident, $gen:ty, $name:ident) => {
        fn $name<S: Into<$gen>>(val: S) -> Self {
            Self::$variant (val.into().into())
        }
    };
}

macro_rules! impl_const_base_val_new_sv {
    ($variant:ident, $gen:ty, $scalar:ident $(, $prefix:ident)?) => {
        paste! {
            impl_const_base_val_internal!($variant, $gen, [<new_ $scalar>]);
            impl_const_base_val_internal!($variant, Vector2<$gen>, [<new_ $($prefix)? vec2>]);
            impl_const_base_val_internal!($variant, Vector3<$gen>, [<new_ $($prefix)? vec3>]);
            impl_const_base_val_internal!($variant, Vector4<$gen>, [<new_ $($prefix)? vec4>]);
        }
    };
}

macro_rules! impl_const_base_val_new_m {
    ($variant:ident, $gen:ty $(, $prefix:ident)?) => {
        paste! {
            impl_const_base_val_internal!($variant, Matrix2<$gen>, [<new_ $($prefix)? mat2>]);
            impl_const_base_val_internal!($variant, Matrix2x3<$gen>, [<new_ $($prefix)? mat23>]);
            impl_const_base_val_internal!($variant, Matrix2x4<$gen>, [<new_ $($prefix)? mat24>]);
            impl_const_base_val_internal!($variant, Matrix3x2<$gen>, [<new_ $($prefix)? mat32>]);
            impl_const_base_val_internal!($variant, Matrix3<$gen>, [<new_ $($prefix)? mat3>]);
            impl_const_base_val_internal!($variant, Matrix3x4<$gen>, [<new_ $($prefix)? mat34>]);
            impl_const_base_val_internal!($variant, Matrix4x2<$gen>, [<new_ $($prefix)? mat42>]);
            impl_const_base_val_internal!($variant, Matrix4x3<$gen>, [<new_ $($prefix)? mat43>]);
            impl_const_base_val_internal!($variant, Matrix4<$gen>, [<new_ $($prefix)? mat4>]);
        }
    };
}

macro_rules! impl_from_to_const_base_val_internal {
    ($to:ty, $variant:ident) => {
        impl From<$to> for ConstBaseVal {
            fn from(v: $to) -> Self {
                ConstBaseVal::$variant(v.into())
            }
        }
        impl TryFrom<ConstBaseVal> for $to {
            type Error = ();

            fn try_from(value: ConstBaseVal) -> Result<Self, Self::Error> {
                match value {
                    ConstBaseVal::$variant(v) => v.try_into().ok().ok_or(()),
                    _ => Err(()),
                }
            }
        }
    };
}

macro_rules! impl_from_to_const_base_val_sv {
    ($gen:ty, $variant:ident) => {
        impl_from_to_const_base_val_internal!($gen, $variant);
        impl_from_to_const_base_val_internal!(Vector2<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Vector3<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Vector4<$gen>, $variant);
        impl_from_to_const_base_val_internal!(ConstSVVal<$gen>, $variant);
    };
}

macro_rules! impl_from_to_const_base_val_m {
    ($gen:ty, $variant:ident) => {
        impl_from_to_const_base_val_internal!(Matrix2<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix2x3<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix2x4<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix3x2<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix3<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix3x4<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix4x2<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix4x3<$gen>, $variant);
        impl_from_to_const_base_val_internal!(Matrix4<$gen>, $variant);
        impl_from_to_const_base_val_internal!(ConstMVal<$gen>, $variant);
        impl_from_to_const_base_val_internal!(ConstSVMVal<$gen>, $variant);
    };
}

/// A generic constant basic value.
#[derive(Clone, PartialEq, Debug)]
pub enum ConstBaseVal {
    Bool(ConstSVVal<bool>),
    Int(ConstSVVal<i32>),
    UInt(ConstSVVal<u32>),
    Float(ConstSVMVal<f32>),
    Double(ConstSVMVal<f64>),
}

impl ConstBaseVal {
    impl_const_base_val_new_sv!(Bool, bool, bool, b);
    impl_const_base_val_new_sv!(Int, i32, int, i);
    impl_const_base_val_new_sv!(UInt, u32, uint, u);
    impl_const_base_val_new_sv!(Float, f32, float);
    impl_const_base_val_new_m!(Float, f32);
    impl_const_base_val_new_sv!(Double, f64, double, d);
    impl_const_base_val_new_m!(Double, f64, d);

    fn test() {
        Self::new_bool(true);
        Self::new_bvec2(Vector2::from_element(true));
    }

    pub fn get_shape(&self) -> BaseTypeShape {
        match self {
            ConstBaseVal::Bool(v) => v.get_shape(),
            ConstBaseVal::Int(v) => v.get_shape(),
            ConstBaseVal::UInt(v) => v.get_shape(),
            ConstBaseVal::Float(v) => v.get_shape(),
            ConstBaseVal::Double(v) => v.get_shape(),
        }
    }

    pub fn type_specifier(&self) -> TypeSpecifier {
        TypeSpecifier::new(self.type_specifier_non_array())
    }

    pub fn type_specifier_non_array(&self) -> TypeSpecifierNonArray {
        match self {
            Self::Bool(ConstSVVal::Scalar(_)) => TypeSpecifierNonArray::Bool,
            Self::Bool(ConstSVVal::Vector(ConstVVal::Vec2(_))) => TypeSpecifierNonArray::BVec2,
            Self::Bool(ConstSVVal::Vector(ConstVVal::Vec3(_))) => TypeSpecifierNonArray::BVec3,
            Self::Bool(ConstSVVal::Vector(ConstVVal::Vec4(_))) => TypeSpecifierNonArray::BVec4,
            Self::Int(ConstSVVal::Scalar(_)) => TypeSpecifierNonArray::Int,
            Self::Int(ConstSVVal::Vector(ConstVVal::Vec2(_))) => TypeSpecifierNonArray::IVec2,
            Self::Int(ConstSVVal::Vector(ConstVVal::Vec3(_))) => TypeSpecifierNonArray::IVec3,
            Self::Int(ConstSVVal::Vector(ConstVVal::Vec4(_))) => TypeSpecifierNonArray::IVec4,
            Self::UInt(ConstSVVal::Scalar(_)) => TypeSpecifierNonArray::UInt,
            Self::UInt(ConstSVVal::Vector(ConstVVal::Vec2(_))) => TypeSpecifierNonArray::UVec2,
            Self::UInt(ConstSVVal::Vector(ConstVVal::Vec3(_))) => TypeSpecifierNonArray::UVec3,
            Self::UInt(ConstSVVal::Vector(ConstVVal::Vec4(_))) => TypeSpecifierNonArray::UVec4,
            Self::Float(ConstSVMVal::Scalar(_)) => TypeSpecifierNonArray::Float,
            Self::Float(ConstSVMVal::Vector(ConstVVal::Vec2(_))) => TypeSpecifierNonArray::Vec2,
            Self::Float(ConstSVMVal::Vector(ConstVVal::Vec3(_))) => TypeSpecifierNonArray::Vec3,
            Self::Float(ConstSVMVal::Vector(ConstVVal::Vec4(_))) => TypeSpecifierNonArray::Vec4,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat2(_))) => TypeSpecifierNonArray::Mat2,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat23(_))) => TypeSpecifierNonArray::Mat23,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat24(_))) => TypeSpecifierNonArray::Mat24,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat32(_))) => TypeSpecifierNonArray::Mat32,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat3(_))) => TypeSpecifierNonArray::Mat3,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat34(_))) => TypeSpecifierNonArray::Mat34,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat42(_))) => TypeSpecifierNonArray::Mat42,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat43(_))) => TypeSpecifierNonArray::Mat43,
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat4(_))) => TypeSpecifierNonArray::Mat4,
            Self::Double(ConstSVMVal::Scalar(_)) => TypeSpecifierNonArray::Double,
            Self::Double(ConstSVMVal::Vector(ConstVVal::Vec2(_))) => TypeSpecifierNonArray::DVec2,
            Self::Double(ConstSVMVal::Vector(ConstVVal::Vec3(_))) => TypeSpecifierNonArray::DVec3,
            Self::Double(ConstSVMVal::Vector(ConstVVal::Vec4(_))) => TypeSpecifierNonArray::DVec4,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat2(_))) => TypeSpecifierNonArray::DMat2,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat23(_))) => TypeSpecifierNonArray::DMat23,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat24(_))) => TypeSpecifierNonArray::DMat24,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat32(_))) => TypeSpecifierNonArray::DMat32,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat3(_))) => TypeSpecifierNonArray::DMat3,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat34(_))) => TypeSpecifierNonArray::DMat34,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat42(_))) => TypeSpecifierNonArray::DMat42,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat43(_))) => TypeSpecifierNonArray::DMat43,
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat4(_))) => TypeSpecifierNonArray::DMat4,
        }
    }

    pub fn as_expr(&self) -> Expr {
        match self {
            Self::Bool(ConstSVVal::Scalar(v)) => Expr::BoolConst(*v),
            Self::Bool(ConstSVVal::Vector(ConstVVal::Vec2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("bvec2")), v.iter().cloned().map(Expr::BoolConst).collect()),
            Self::Bool(ConstSVVal::Vector(ConstVVal::Vec3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("bvec3")), v.iter().cloned().map(Expr::BoolConst).collect()),
            Self::Bool(ConstSVVal::Vector(ConstVVal::Vec4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("bvec4")), v.iter().cloned().map(Expr::BoolConst).collect()),
            Self::Int(ConstSVVal::Scalar(v)) => Expr::IntConst(*v),
            Self::Int(ConstSVVal::Vector(ConstVVal::Vec2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("ivec2")), v.iter().cloned().map(Expr::IntConst).collect()),
            Self::Int(ConstSVVal::Vector(ConstVVal::Vec3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("ivec3")), v.iter().cloned().map(Expr::IntConst).collect()),
            Self::Int(ConstSVVal::Vector(ConstVVal::Vec4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("ivec4")), v.iter().cloned().map(Expr::IntConst).collect()),
            Self::UInt(ConstSVVal::Scalar(v)) => Expr::UIntConst(*v),
            Self::UInt(ConstSVVal::Vector(ConstVVal::Vec2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("uvec2")), v.iter().cloned().map(Expr::UIntConst).collect()),
            Self::UInt(ConstSVVal::Vector(ConstVVal::Vec3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("uvec3")), v.iter().cloned().map(Expr::UIntConst).collect()),
            Self::UInt(ConstSVVal::Vector(ConstVVal::Vec4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("uvec4")), v.iter().cloned().map(Expr::UIntConst).collect()),
            Self::Float(ConstSVMVal::Scalar(v)) => Expr::FloatConst(*v),
            Self::Float(ConstSVMVal::Vector(ConstVVal::Vec2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("vec2")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Vector(ConstVVal::Vec3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("vec3")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Vector(ConstVVal::Vec4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("vec4")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat2")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat23(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat23")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat24(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat24")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat32(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat32")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat3")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat34(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat34")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat42(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat42")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat43(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat43")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Float(ConstSVMVal::Matrix(ConstMVal::Mat4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("mat4")), v.iter().cloned().map(Expr::FloatConst).collect()),
            Self::Double(ConstSVMVal::Scalar(v)) => Expr::DoubleConst(*v),
            Self::Double(ConstSVMVal::Vector(ConstVVal::Vec2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dvec4")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Vector(ConstVVal::Vec3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dvec4")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Vector(ConstVVal::Vec4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dvec4")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat2(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat2")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat23(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat23")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat24(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat24")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat32(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat32")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat3(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat3")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat34(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat34")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat42(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat42")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat43(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat43")), v.iter().cloned().map(Expr::DoubleConst).collect()),
            Self::Double(ConstSVMVal::Matrix(ConstMVal::Mat4(v))) =>
                Expr::FunCall(FunIdentifier::Identifier(Identifier::from("dmat4")), v.iter().cloned().map(Expr::DoubleConst).collect()),
        }
    }
}

impl_from_to_const_base_val_sv!(bool, Bool);
impl_from_to_const_base_val_sv!(i32, Int);
impl_from_to_const_base_val_sv!(u32, UInt);
impl_from_to_const_base_val_sv!(f32, Float);
impl_from_to_const_base_val_m!(f32, Float);
impl_from_to_const_base_val_sv!(f64, Double);
impl_from_to_const_base_val_m!(f64, Double);

#[derive(Clone, PartialEq, Debug)]
pub struct ConstArray {
    type_specifier: TypeSpecifierNonArray,
    dims: Box<[u32]>,
    data: Box<[ConstVal]>,
}

impl ConstArray {
    pub fn type_specifier(&self) -> TypeSpecifier {
        let array_specifier = ArraySpecifier {
            dimensions: NonEmpty::from_non_empty_iter(self.dims.iter().cloned().map(Expr::UIntConst).map(Box::new).map(ArraySpecifierDimension::ExplicitlySized)).unwrap(),
        };

        TypeSpecifier {
            ty: self.type_specifier.clone(),
            array_specifier: Some(array_specifier),
        }
    }

    pub fn get(&self, indices: &[u32]) -> Option<&ConstVal> {
        if self.dims.len() != indices.len() {
            None
        } else {
            let mut index = 0usize;
            let mut cumulative = 1usize;

            for i in 0..self.dims.len() {
                if indices[i] > self.dims[i] {
                    return None;
                }

                index += cumulative * (indices[i] as usize);
                cumulative *= (self.dims[i] as usize);
            }

            self.data.get(index)
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct ConstStruct {
    type_specifier: StructSpecifier,
    entries: HashMap<String, ConstVal>,
}

impl ConstStruct {
    pub fn type_specifier(&self) -> TypeSpecifier {
        TypeSpecifier::new(TypeSpecifierNonArray::Struct(self.type_specifier.clone()))
    }
}

impl ConstLookup for ConstStruct {
    fn lookup_const(&self, ident: &Identifier) -> Option<&ConstVal> {
        self.entries.get(&ident.0)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum ConstVal {
    Base(ConstBaseVal),
    Array(ConstArray),
    Struct(ConstStruct),
}

impl ConstVal {
    pub fn try_into_base(&self) -> Option<&ConstBaseVal> {
        match self {
            ConstVal::Base(v) => Some(v),
            _ => None,
        }
    }

    pub fn type_specifier(&self) -> TypeSpecifier {
        match self {
            ConstVal::Base(v) => v.type_specifier(),
            ConstVal::Array(v) => v.type_specifier(),
            ConstVal::Struct(v) => v.type_specifier(),
        }
    }

    pub fn as_expr(&self) -> Expr {
        todo!()
    }
}

impl<T: Into<ConstBaseVal>> From<T> for ConstVal {
    fn from(v: T) -> Self {
        ConstVal::Base(v.into())
    }
}

pub enum ConstValOrExpr {
    Const(ConstVal),
    Expr(Expr),
}

impl<T: Into<ConstVal>> From<T> for ConstValOrExpr {
    fn from(val: T) -> Self {
        Self::Const(val.into())
    }
}

impl From<Expr> for ConstValOrExpr {
    fn from(expr: Expr) -> Self {
        Self::Expr(expr)
    }
}

impl From<ConstValOrExpr> for Expr {
    fn from(v: ConstValOrExpr) -> Self {
        match v {
            ConstValOrExpr::Const(v) => v.as_expr(),
            ConstValOrExpr::Expr(expr) => expr
        }
    }
}

/// Propagates const values in a expression creating a either a new expression or a constant value.
///
/// No other transformation besides constant evaluation will be applied.
pub fn const_propagate_expr<CL: ConstLookup, FL: ConstEvalFunctionLookup>(expr: &Expr, cl: &CL, fl: &FL) -> Result<ConstValOrExpr, ConstEvalError> {
    match expr {
        Expr::Variable(ident) => Ok(cl.lookup_const(ident).cloned().map(ConstValOrExpr::from).unwrap_or_else(|| Expr::Variable(ident.clone()).into())),
        Expr::IntConst(v) => Ok(ConstBaseVal::new_int(*v).into()),
        Expr::UIntConst(v) => Ok(ConstBaseVal::new_uint(*v).into()),
        Expr::BoolConst(v) => Ok(ConstBaseVal::new_bool(*v).into()),
        Expr::FloatConst(v) => Ok(ConstBaseVal::new_float(*v).into()),
        Expr::DoubleConst(v) => Ok(ConstBaseVal::new_double(*v).into()),
        Expr::Unary(op, a) => {
            let a = const_propagate_expr(a, cl, fl)?;
            let a = match a {
                ConstValOrExpr::Const(v) => v,
                ConstValOrExpr::Expr(expr) => return Ok(Expr::Unary(op.clone(), Box::new(expr)).into())
            };

            let a_ty = a.type_specifier();
            let err = || ConstEvalError::IllegalUnaryOperand(op.clone(), a_ty.clone());
            let a = a.try_into_base().ok_or_else(err)?;
            match op {
                UnaryOp::Inc => Err(ConstEvalError::UnaryOpExpectedLValue(UnaryOp::Inc)),
                UnaryOp::Dec => Err(ConstEvalError::UnaryOpExpectedLValue(UnaryOp::Dec)),
                UnaryOp::Add => function::OP_UNARY_ADD.eval(&[a]).map(ConstValOrExpr::from).ok_or_else(err),
                UnaryOp::Minus => function::OP_UNARY_MINUS.eval(&[a]).map(ConstValOrExpr::from).ok_or_else(err),
                UnaryOp::Not => function::OP_UNARY_NOT.eval(&[a]).map(ConstValOrExpr::from).ok_or_else(err),
                UnaryOp::Complement => function::OP_UNARY_COMPLEMENT.eval(&[a]).map(ConstValOrExpr::from).ok_or_else(err),
            }
        },
        Expr::Binary(op, a, b) => {
            let a = const_propagate_expr(a, cl, fl)?;
            let b = const_propagate_expr(b, cl, fl)?;
            let (a, b) = match (a, b) {
                (ConstValOrExpr::Const(a), ConstValOrExpr::Const(b)) => (a, b),
                (a, b) => return Ok(Expr::Binary(op.clone(), Box::new(a.into()), Box::new(b.into())).into()),
            };

            // TODO eq and neq on arrays and structs
            let (a_ty, b_ty) = (a.type_specifier(), b.type_specifier());
            let err = || ConstEvalError::IllegalBinaryOperand(op.clone(), a_ty.clone(), b_ty.clone());
            let (a, b) = (a.try_into_base().ok_or_else(err)?, b.try_into_base().ok_or_else(err)?);
            match op {
                BinaryOp::Or => function::OP_BINARY_OR.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Xor => function::OP_BINARY_XOR.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::And => function::OP_BINARY_AND.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::BitOr => function::OP_BINARY_BIT_OR.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::BitXor => function::OP_BINARY_BIT_XOR.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::BitAnd => function::OP_BINARY_BIT_AND.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Equal => function::OP_BINARY_EQUAL.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::NonEqual => {
                    function::OP_BINARY_EQUAL.eval(&[a, b]).map(|v| {
                        let val: bool = v.try_into().expect("OP_BINARY_EQUAL did not return bool scalar");
                        Some(ConstBaseVal::new_bool(!val))
                    }).flatten().map(ConstValOrExpr::from).ok_or_else(err)
                },
                BinaryOp::LT => function::OP_BINARY_LT.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::GT => function::OP_BINARY_GT.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::LTE => function::OP_BINARY_LTE.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::GTE => function::OP_BINARY_GTE.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::LShift => function::OP_BINARY_LSHIFT.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::RShift => function::OP_BINARY_RSHIFT.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Add => function::OP_BINARY_ADD.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Sub => function::OP_BINARY_SUB.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Mult => function::OP_BINARY_MULT.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Div => function::OP_BINARY_DIV.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
                BinaryOp::Mod => function::OP_BINARY_MOD.eval(&[a, b]).map(ConstValOrExpr::from).ok_or_else(err),
            }
        },
        Expr::Ternary(a, b, c) => {
            match const_propagate_expr(a, cl, fl)? {
                ConstValOrExpr::Const(ConstVal::Base(ConstBaseVal::Bool(ConstSVVal::Scalar(v)))) => {
                    let selected = if v { b } else { c };
                    const_propagate_expr(selected, cl, fl)
                }
                ConstValOrExpr::Const(_) => Err(ConstEvalError::TernaryExpectedScalarBool),
                ConstValOrExpr::Expr(expr) => {
                    let b = const_propagate_expr(b, cl, fl)?;
                    let c = const_propagate_expr(c, cl, fl)?;
                    Ok(Expr::Ternary(Box::new(expr), Box::new(b.into()), Box::new(c.into())).into())
                }
            }
        },
        Expr::Assignment(a, op, b) => {
            let a = match const_propagate_expr(a, cl, fl)? {
                ConstValOrExpr::Const(_) => return Err(ConstEvalError::AssignmentExpectedLValue),
                ConstValOrExpr::Expr(expr) => expr,
            };
            let b = const_propagate_expr(b, cl, fl)?;
            Ok(Expr::Assignment(Box::new(a), op.clone(), Box::new(b.into())).into())
        },
        Expr::Bracket(_, _) => todo!(),
        Expr::FunCall(ident, params) => {
            // Generates the propagated expression when called
            let ret = |p: Vec<ConstValOrExpr>| Ok(Expr::FunCall(ident.clone(), p.into_iter().map(Expr::from).collect()).into());

            let params = params.iter().map(|e| const_propagate_expr(e, cl, fl)).collect::<Result<Vec<_>, ConstEvalError>>()?;
            let param_ref = params.iter().map(|v| match v {
                ConstValOrExpr::Const(ConstVal::Base(b)) => Some(b),
                _ => None,
            }).collect::<Option<Vec<_>>>();
            let param_ref = match param_ref {
                Some(v) => v,
                None => return ret(params),
            };

            let func = match ident {
                FunIdentifier::Identifier(ident) => match fl.lookup(ident) {
                    Some(func) => func,
                    None => return ret(params),
                },
                FunIdentifier::Expr(_) => return ret(params),
            };
            func.eval(&param_ref).map(ConstVal::Base).map(ConstValOrExpr::from).ok_or(ConstEvalError::NoMatchingFunctionOverload)
        },
        Expr::Dot(a, ident) => {
            let a = const_propagate_expr(a, cl, fl)?;
            let a = match a {
                ConstValOrExpr::Const(v) => v,
                ConstValOrExpr::Expr(expr) => return Ok(Expr::Dot(Box::new(expr), ident.clone()).into()),
            };

            match a {
                ConstVal::Struct(str) => str.lookup_const(ident).cloned().map(ConstValOrExpr::from).ok_or_else(|| ConstEvalError::UnknownStructureMember(ident.0.clone())),
                ConstVal::Array(a) => todo!(),
                _ => Err(ConstEvalError::DotStructureRequired),
            }
        },
        Expr::PostInc(a) => {
            match const_propagate_expr(a, cl, fl)? {
                ConstValOrExpr::Const(_) => Err(ConstEvalError::PostOpExpectedLValue),
                ConstValOrExpr::Expr(expr) => Ok(Expr::PostInc(Box::new(expr)).into()),
            }
        },
        Expr::PostDec(a) => {
            match const_propagate_expr(a, cl, fl)? {
                ConstValOrExpr::Const(_) => Err(ConstEvalError::PostOpExpectedLValue),
                ConstValOrExpr::Expr(expr) => Ok(Expr::PostDec(Box::new(expr)).into()),
            }
        },
        Expr::Comma(a, b) => {
            let a = const_propagate_expr(a, cl, fl)?;
            let b = const_propagate_expr(b, cl, fl)?;
            Ok(Expr::Comma(Box::new(a.into()), Box::new(b.into())).into())
        },
    }
}

#[derive(Clone, PartialEq, Debug)]
#[non_exhaustive]
pub enum ConstEvalError {
    UnaryOpExpectedLValue(UnaryOp),
    UnknownStructureMember(String),
    DotStructureRequired,
    TernaryExpectedScalarBool,
    AssignmentExpectedLValue,
    PostOpExpectedLValue,
    UnknownIdentifier(String),
    IllegalExpression,
    IllegalUnaryOp(UnaryOp),
    IllegalUnaryOperand(UnaryOp, TypeSpecifier),
    IllegalBinaryOp(BinaryOp),
    IllegalBinaryOperand(BinaryOp, TypeSpecifier, TypeSpecifier),
    NoMatchingFunctionOverload,
}

struct ScopedConstLookup {
    scopes: Vec<HashMap<String, ConstVal>>,
}

impl ScopedConstLookup {
    pub fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    pub fn set_value(&mut self, name: &Identifier, value: ConstVal) {
        self.scopes.last_mut().unwrap().insert(name.0.clone(), value);
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }
}

impl ConstLookup for ScopedConstLookup {
    fn lookup_const(&self, ident: &Identifier) -> Option<&ConstVal> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get(&ident.0) {
                return Some(val)
            }
        }
        None
    }
}

struct ConstPropagateVisitor {
    value_lookup: ScopedConstLookup,
}

impl VisitorMut for ConstPropagateVisitor {
    fn visit_external_declaration(&mut self, decl: &mut ExternalDeclaration) -> Visit {
        if let ExternalDeclaration::Declaration(decl) = decl {
            match decl {
                Declaration::FunctionPrototype(_) => {}
                Declaration::InitDeclaratorList(init) => {
                    todo!()
                }
                Declaration::Block(_) => {}
                Declaration::Global(_, _) => {}
                _ => {}
            }
        }

        todo!()
    }
}

pub fn const_propagate_unit(shader: &mut TranslationUnit) {

}

mod function {
    use std::any::TypeId;
    use std::cmp::Ordering;
    use std::collections::HashMap;
    use std::marker::PhantomData;

    use std::ops::{BitAnd, BitOr, BitXor, Neg, Not};
    use glsl::syntax::{Identifier, NonEmpty};

    use lazy_static::lazy_static;

    use nalgebra::{ArrayStorage, Const, DimName, Matrix, Matrix2, Matrix2x3, Matrix2x4, Matrix3, Matrix3x2, Matrix3x4, Matrix4, Matrix4x2, Matrix4x3, Scalar, U1, Vector, Vector2, Vector3, Vector4};
    use num_traits::{One, Zero};

    use super::{ConstEvalFunctionLookup, ConstGenericValue, ConstGenericMappable, ConstGenericZipMappable};
    use super::{BaseTypeShape, ConstBaseVal, ConstMVal, ConstSVMVal, ConstSVVal};

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
    pub enum ParameterBaseType {
        Bool,
        Int,
        UInt,
        Float,
        Double,
    }

    impl ParameterBaseType {
        pub fn from_const_val(val: &ConstBaseVal) -> Self {
            match val {
                ConstBaseVal::Bool(_) => Self::Bool,
                ConstBaseVal::Int(_) => Self::Int,
                ConstBaseVal::UInt(_) => Self::UInt,
                ConstBaseVal::Float(_) => Self::Float,
                ConstBaseVal::Double(_) => Self::Double,
            }
        }

        /// Ordered by glsl implicit casting rules. If a < b then a can be implicitly cast to b.
        pub fn cast_cmp(&self, other: &Self) -> Option<Ordering> {
            if self == other {
                Some(Ordering::Equal)
            } else {
                match (self, other) {
                    (Self::Int, Self::UInt) |
                    (Self::Int, Self::Float) |
                    (Self::Int, Self::Double) |
                    (Self::UInt, Self::Float) |
                    (Self::UInt, Self::Double) |
                    (Self::Float, Self::Double) => Some(Ordering::Less),
                    (Self::UInt, Self::Int) |
                    (Self::Float, Self::Int) |
                    (Self::Double, Self::Int) |
                    (Self::Float, Self::UInt) |
                    (Self::Double, Self::UInt) |
                    (Self::Double, Self::Float) => Some(Ordering::Greater),
                    _ => None,
                }
            }
        }

        pub fn can_cast_into(&self, other: &Self) -> bool {
            match self.cast_cmp(other) {
                Some(Ordering::Less) |
                Some(Ordering::Equal) => true,
                _ => false
            }
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug)]
    pub enum ParameterShape {
        Scalar,
        Vec2,
        Vec3,
        Vec4,
        GenericSV,
        Mat2,
        Mat23,
        Mat24,
        Mat32,
        Mat3,
        Mat34,
        Mat42,
        Mat43,
        Mat4,
        GenericM,
        GenericSVM,
    }

    impl ParameterShape {
        pub fn matches(&self, val: BaseTypeShape) -> bool {
            match self {
                ParameterShape::Scalar => val.is_scalar(),
                ParameterShape::Vec2 => val == BaseTypeShape::Vec2,
                ParameterShape::Vec3 => val == BaseTypeShape::Vec3,
                ParameterShape::Vec4 => val == BaseTypeShape::Vec4,
                ParameterShape::GenericSV => val.is_scalar() || val.is_vector(),
                ParameterShape::Mat2 => val == BaseTypeShape::Mat2,
                ParameterShape::Mat23 => val == BaseTypeShape::Mat23,
                ParameterShape::Mat24 => val == BaseTypeShape::Mat24,
                ParameterShape::Mat32 => val == BaseTypeShape::Mat32,
                ParameterShape::Mat3 => val == BaseTypeShape::Mat3,
                ParameterShape::Mat34 => val == BaseTypeShape::Mat34,
                ParameterShape::Mat42 => val == BaseTypeShape::Mat42,
                ParameterShape::Mat43 => val == BaseTypeShape::Mat43,
                ParameterShape::Mat4 => val == BaseTypeShape::Mat4,
                ParameterShape::GenericM => val.is_vector(),
                ParameterShape::GenericSVM => true,
            }
        }
    }

    #[derive(Copy, Clone, Eq, PartialEq, Debug)]
    pub struct ParameterType {
        base_type: ParameterBaseType,
        shape: ParameterShape,
    }

    impl ParameterType {
        pub fn new(base_type: ParameterBaseType, shape: ParameterShape) -> Self {
            Self {
                base_type,
                shape,
            }
        }

        /// Compare function of the glsl implicit casting rules. For any a and b where a can be
        /// implicitly cast to b, a < b.
        ///
        /// Note that a < b does not imply that a can be cast to b. Assume we have a = (Int, Vec2),
        /// b = (UInt, Vec2), c = (bool, Vec2). Then all of the following orderings are valid:
        /// \[c, a, b], \[a, c, b] and \[a, b, c], since c cannot be cast to a or b and vice versa.
        /// It is only guaranteed that the order of c is consistent at runtime.
        pub fn cast_cmp(&self, other: &Self) -> Ordering {
            let shape_ord = self.shape.cmp(&other.shape);
            if shape_ord == Ordering::Equal {
                self.base_type.partial_cmp(&other.base_type).unwrap_or_else(||
                    self.base_type.cmp(&other.base_type)
                )
            } else {
                shape_ord
            }
        }
    }

    /// A instance of a const evaluable function. It has a fixed prototype and can be called to
    /// evaluate some parameters matching the prototype.
    pub struct ConstEvalFunctionInstance {
        prototype: Option<Box<[ParameterType]>>,
        function: Box<dyn Fn(&[&ConstBaseVal]) -> Option<ConstBaseVal> + Send + Sync>,
    }

    impl ConstEvalFunctionInstance {
        pub fn from_generic<F>(f: F) -> Self where F: Fn(&[&ConstBaseVal]) -> Option<ConstBaseVal> + Send + Sync + 'static {
            let function = Box::new(f);

            Self {
                prototype: None,
                function,
            }
        }

        pub fn from_fn_0<R, F>(f: F) -> Self where R: ConstParameter, F: Fn() -> R + Send + Sync + 'static {
            let prototype = Some(Box::new([]) as Box<[ParameterType]>);
            let function = Box::new(move |params: &[&ConstBaseVal]| {
                if params.len() != 0 {
                    panic!("Parameter list length mismatch. Expected 0 but got {:?}", params.len());
                } else {
                    Some(f().into_const_base_val())
                }
            });

            Self {
                prototype,
                function
            }
        }

        pub fn from_fn_1<R, T0, F>(f: F) -> Self where R: ConstParameter, T0: ConstParameter + 'static, F: Fn(T0) -> Option<R> + Send + Sync + 'static {
            let prototype = Some(Box::new([T0::get_type()]) as Box<[ParameterType]>);
            let function = Box::new(move |params: &[&ConstBaseVal]| {
                if params.len() != 1 {
                    panic!("Parameter list length mismatch. Expected 1 but got {:?}", params.len());
                } else {
                    let t0 = T0::try_cast_from(params[0]).unwrap_or_else(|| panic!("Implicit cast failed: {:?} to {:?}", params[0].type_specifier(), TypeId::of::<T0>()));
                    f(t0).map(R::into_const_base_val)
                }
            });

            Self {
                prototype,
                function
            }
        }

        pub fn from_fn_2<R, T0, T1, F>(f: F) -> Self where R: ConstParameter, T0: ConstParameter + 'static, T1: ConstParameter + 'static, F: Fn(T0, T1) -> Option<R> + Send + Sync + 'static {
            let prototype = Some(Box::new([T0::get_type(), T1::get_type()]) as Box<[ParameterType]>);
            let function = Box::new(move |params: &[&ConstBaseVal]| {
                if params.len() != 2 {
                    panic!("Parameter list length mismatch. Expected 2 but got {:?}", params.len());
                } else {
                    let t0 = T0::try_cast_from(params[0]).unwrap_or_else(|| panic!("Implicit cast failed: {:?} to {:?}", params[0].type_specifier(), TypeId::of::<T0>()));
                    let t1 = T1::try_cast_from(params[1]).unwrap_or_else(|| panic!("Implicit cast failed: {:?} to {:?}", params[1].type_specifier(), TypeId::of::<T0>()));
                    f(t0, t1).map(R::into_const_base_val)
                }
            });

            Self {
                prototype,
                function
            }
        }

        /// Checks if the provided parameter types are compatible with this function prototype.
        /// This check includes implicit casting rules.
        ///
        /// For example if the prototype is \[(Vec2, UInt)] calling this function with
        /// \[(Vec2, Int)] or \[(Vec2, UInt)] returns true while calling it with \[(Vec2, Bool)]
        /// returns false.
        pub fn compatible_with(&self, params: &[(BaseTypeShape, ParameterBaseType)]) -> bool {
            if let Some(prototype) = &self.prototype {
                if params.len() != prototype.len() {
                    return false;
                }

                for ((size, base_type), proto) in params.iter().zip(prototype.iter()) {
                    if !proto.shape.matches(*size) {
                        return false;
                    }
                    if !base_type.can_cast_into(&proto.base_type) {
                        return false;
                    }
                }
                true
            } else {
                true
            }
        }

        /// Evaluates this function for the provided parameters performing implicit casting if
        /// necessary.
        ///
        /// # Panics
        /// If the provided parameters cannot be implicitly cast to the required type. Check
        /// compatibility with [Overload::compatible_with] first if needed.
        pub fn eval(&self, params: &[&ConstBaseVal]) -> Option<ConstBaseVal> {
            (self.function)(params)
        }

        /// Provides a order sorting functions by prototype specificity and casting order.
        ///
        /// The practical goal is that if a list of functions is sorted by this order then one can
        /// iterate this list in ascending order and the first function compatible with the provided
        /// parameters will also be the best matching function.
        pub fn cast_cmp(&self, other: &Self) -> Ordering {
            match (&self.prototype, &other.prototype) {
                (Some(p1), Some(p2)) => {
                    let len_cmp = p1.len().cmp(&p2.len());
                    if len_cmp == Ordering::Equal {
                        p1.iter().zip(p2.iter()).fold(Ordering::Equal, |i, (a, b)| {
                            if i == Ordering::Equal {
                                a.cast_cmp(b)
                            } else {
                                i
                            }
                        })
                    } else {
                        len_cmp
                    }
                },
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (None, None) => Ordering::Equal,
            }
        }
    }

    pub struct ConstEvalFunctionBuilder {
        overloads: Vec<ConstEvalFunctionInstance>,
    }

    impl ConstEvalFunctionBuilder {
        pub fn new() -> Self {
            Self {
                overloads: Vec::new(),
            }
        }

        pub fn add_generic<F>(mut self, f: F) -> Self where F: Fn(&[&ConstBaseVal]) -> Option<ConstBaseVal> + Send + Sync + 'static {
            self.overloads.push(ConstEvalFunctionInstance::from_generic(f));
            self
        }

        /// Adds an overload to this function taking no parameters.
        pub fn add_overload_0<R, F>(mut self, f: F) -> Self where R: ConstParameter, F: Fn() -> R + Send + Sync + 'static {
            self.overloads.push(ConstEvalFunctionInstance::from_fn_0(f));
            self
        }

        /// Adds an overload to this function taking 1 parameter.
        ///
        /// If the provided function returns [`None`] when evaluated it is not interpreted as an
        /// error but indicates that the parameters do not match the function prototype (for example
        /// when using generic sized vectors/matrices). The [ConstEvalFunction::eval] method will
        /// not immediately return but continue searching for a matching overload if a function
        /// returns [`None`].
        pub fn add_overload_1<R, T0, F>(mut self, f: F) -> Self where R: ConstParameter, T0: ConstParameter + 'static, F: Fn(T0) -> Option<R> + Send + Sync + 'static {
            self.overloads.push(ConstEvalFunctionInstance::from_fn_1(f));
            self
        }

        /// Adds an overload to this function taking 2 parameter.
        ///
        /// If the provided function returns [`None`] when evaluated it is not interpreted as an
        /// error but indicates that the parameters do not match the function prototype (for example
        /// when using generic sized vectors/matrices). The [ConstEvalFunction::eval] method will
        /// not immediately return but continue searching for a matching overload if a function
        /// returns [`None`].
        pub fn add_overload_2<R, T0, T1, F>(mut self, f: F) -> Self where R: ConstParameter, T0: ConstParameter + 'static, T1: ConstParameter + 'static, F: Fn(T0, T1) -> Option<R> + Send + Sync + 'static {
            self.overloads.push(ConstEvalFunctionInstance::from_fn_2(f));
            self
        }

        pub fn build(mut self) -> ConstEvalFunction {
            self.overloads.sort_by(ConstEvalFunctionInstance::cast_cmp);

            ConstEvalFunction {
                overloads: self.overloads.into_boxed_slice(),
            }
        }
    }

    pub struct ConstEvalFunction {
        overloads: Box<[ConstEvalFunctionInstance]>,
    }

    impl ConstEvalFunction {
        /// Evaluates the function on the provided parameters. Returns [`None`] if no matching
        /// overload could be found.
        pub fn eval(&self, params: &[&ConstBaseVal]) -> Option<ConstBaseVal> {
            let mut types = Vec::with_capacity(params.len());
            for param in params {
                types.push((param.get_shape(), ParameterBaseType::from_const_val(param)));
            }

            for func in self.overloads.iter() {
                if func.compatible_with(&types) {
                    if let Some(result) = func.eval(params) {
                        return Some(result);
                    }
                }
            };
            None
        }
    }

    impl ConstEvalFunctionLookup for HashMap<String, ConstEvalFunction> {
        fn lookup(&self, ident: &Identifier) -> Option<&ConstEvalFunction> {
            self.get(&ident.0)
        }
    }

    pub trait ConstParameter: Sized {
        fn get_type() -> ParameterType;

        fn try_cast_from(val: &ConstBaseVal) -> Option<Self>;

        fn into_const_base_val(self) -> ConstBaseVal;
    }

    macro_rules! const_param_bool {
        ($ty:ty, $ps:expr) => {
            impl ConstParameter for $ty {
                fn get_type() -> ParameterType {
                    ParameterType::new(ParameterBaseType::Bool, $ps)
                }

                fn try_cast_from(val: &ConstBaseVal) -> Option<Self> {
                    match val {
                        ConstBaseVal::Bool(v) => v.clone().try_into().ok(),
                        _ => None,
                    }
                }

                fn into_const_base_val(self) -> ConstBaseVal {
                    ConstBaseVal::Bool(self.into())
                }
            }
        };
    }
    const_param_bool!(bool, ParameterShape::Scalar);
    const_param_bool!(Vector2<bool>, ParameterShape::Vec2);
    const_param_bool!(Vector3<bool>, ParameterShape::Vec3);
    const_param_bool!(Vector4<bool>, ParameterShape::Vec4);
    const_param_bool!(ConstSVVal<bool>, ParameterShape::GenericSV);

    macro_rules! const_param_int {
        ($ty:ty, $ps:expr) => {
            impl ConstParameter for $ty {
                fn get_type() -> ParameterType {
                    ParameterType::new(ParameterBaseType::Int, $ps)
                }

                fn try_cast_from(val: &ConstBaseVal) -> Option<Self> {
                    match val {
                        ConstBaseVal::Int(v) => v.clone().try_into().ok(),
                        _ => None,
                    }
                }

                fn into_const_base_val(self) -> ConstBaseVal {
                    ConstBaseVal::Int(self.into())
                }
            }
        };
    }
    const_param_int!(i32, ParameterShape::Scalar);
    const_param_int!(Vector2<i32>, ParameterShape::Vec2);
    const_param_int!(Vector3<i32>, ParameterShape::Vec3);
    const_param_int!(Vector4<i32>, ParameterShape::Vec4);
    const_param_int!(ConstSVVal<i32>, ParameterShape::GenericSV);

    macro_rules! const_param_uint {
        ($ty:ty, $ps:expr) => {
            impl ConstParameter for $ty {
                fn get_type() -> ParameterType {
                    ParameterType::new(ParameterBaseType::UInt, $ps)
                }

                fn try_cast_from(val: &ConstBaseVal) -> Option<Self> {
                    match val {
                        ConstBaseVal::Int(v) => v.map(|v| u32::construct_from(&v)).try_into().ok(),
                        ConstBaseVal::UInt(v) => v.clone().try_into().ok(),
                        _ => None,
                    }
                }

                fn into_const_base_val(self) -> ConstBaseVal {
                    ConstBaseVal::UInt(self.into())
                }
            }
        };
    }
    const_param_uint!(u32, ParameterShape::Scalar);
    const_param_uint!(Vector2<u32>, ParameterShape::Vec2);
    const_param_uint!(Vector3<u32>, ParameterShape::Vec3);
    const_param_uint!(Vector4<u32>, ParameterShape::Vec4);
    const_param_uint!(ConstSVVal<u32>, ParameterShape::GenericSV);

    macro_rules! const_param_float {
        ($ty:ty, $ps:expr) => {
            impl ConstParameter for $ty {
                fn get_type() -> ParameterType {
                    ParameterType::new(ParameterBaseType::Float, $ps)
                }

                fn try_cast_from(val: &ConstBaseVal) -> Option<Self> {
                    match val {
                        ConstBaseVal::Int(v) => ConstSVMVal::from(v.map(|v| f32::construct_from(&v))).try_into().ok(),
                        ConstBaseVal::UInt(v) => ConstSVMVal::from(v.map(|v| f32::construct_from(&v))).try_into().ok(),
                        ConstBaseVal::Float(v) => v.clone().try_into().ok(),
                        _ => None,
                    }
                }

                fn into_const_base_val(self) -> ConstBaseVal {
                    ConstBaseVal::Float(self.into())
                }
            }
        };
    }
    const_param_float!(f32, ParameterShape::Scalar);
    const_param_float!(Vector2<f32>, ParameterShape::Vec2);
    const_param_float!(Vector3<f32>, ParameterShape::Vec3);
    const_param_float!(Vector4<f32>, ParameterShape::Vec4);
    const_param_float!(Matrix2<f32>, ParameterShape::Mat2);
    const_param_float!(Matrix2x3<f32>, ParameterShape::Mat23);
    const_param_float!(Matrix2x4<f32>, ParameterShape::Mat24);
    const_param_float!(Matrix3x2<f32>, ParameterShape::Mat32);
    const_param_float!(Matrix3<f32>, ParameterShape::Mat3);
    const_param_float!(Matrix3x4<f32>, ParameterShape::Mat34);
    const_param_float!(Matrix4x2<f32>, ParameterShape::Mat42);
    const_param_float!(Matrix4x3<f32>, ParameterShape::Mat43);
    const_param_float!(Matrix4<f32>, ParameterShape::Mat4);
    const_param_float!(ConstMVal<f32>, ParameterShape::GenericM);
    const_param_float!(ConstSVVal<f32>, ParameterShape::GenericSV);
    const_param_float!(ConstSVMVal<f32>, ParameterShape::GenericSVM);

    macro_rules! const_param_double {
        ($ty:ty, $ps:expr) => {
            impl ConstParameter for $ty {
                fn get_type() -> ParameterType {
                    ParameterType::new(ParameterBaseType::Double, $ps)
                }

                fn try_cast_from(val: &ConstBaseVal) -> Option<Self> {
                    match val {
                        ConstBaseVal::Int(v) => ConstSVMVal::from(v.map(|v| f64::construct_from(&v))).try_into().ok(),
                        ConstBaseVal::UInt(v) => ConstSVMVal::from(v.map(|v| f64::construct_from(&v))).try_into().ok(),
                        ConstBaseVal::Float(v) => v.map(|v| f64::construct_from(&v)).try_into().ok(),
                        ConstBaseVal::Double(v) => v.clone().try_into().ok(),
                        _ => None,
                    }
                }

                fn into_const_base_val(self) -> ConstBaseVal {
                    ConstBaseVal::Double(self.into())
                }
            }
        };
    }
    const_param_double!(f64, ParameterShape::Scalar);
    const_param_double!(Vector2<f64>, ParameterShape::Vec2);
    const_param_double!(Vector3<f64>, ParameterShape::Vec3);
    const_param_double!(Vector4<f64>, ParameterShape::Vec4);
    const_param_double!(Matrix2<f64>, ParameterShape::Mat2);
    const_param_double!(Matrix2x3<f64>, ParameterShape::Mat23);
    const_param_double!(Matrix2x4<f64>, ParameterShape::Mat24);
    const_param_double!(Matrix3x2<f64>, ParameterShape::Mat32);
    const_param_double!(Matrix3<f64>, ParameterShape::Mat3);
    const_param_double!(Matrix3x4<f64>, ParameterShape::Mat34);
    const_param_double!(Matrix4x2<f64>, ParameterShape::Mat42);
    const_param_double!(Matrix4x3<f64>, ParameterShape::Mat43);
    const_param_double!(Matrix4<f64>, ParameterShape::Mat4);
    const_param_double!(ConstMVal<f64>, ParameterShape::GenericM);
    const_param_double!(ConstSVVal<f64>, ParameterShape::GenericSV);
    const_param_double!(ConstSVMVal<f64>, ParameterShape::GenericSVM);

    trait ScalarConstructFrom<T> {
        fn construct_from(from: &T) -> Self;
    }

    impl ScalarConstructFrom<bool> for bool {
        fn construct_from(from: &bool) -> bool {
            *from
        }
    }

    impl ScalarConstructFrom<i32> for bool {
        fn construct_from(from: &i32) -> bool {
            *from != 0i32
        }
    }

    impl ScalarConstructFrom<u32> for bool {
        fn construct_from(from: &u32) -> bool {
            *from != 0u32
        }
    }

    impl ScalarConstructFrom<f32> for bool {
        fn construct_from(from: &f32) -> bool {
            *from != 0f32
        }
    }

    impl ScalarConstructFrom<f64> for bool {
        fn construct_from(from: &f64) -> bool {
            *from != 0f64
        }
    }

    impl ScalarConstructFrom<bool> for i32 {
        fn construct_from(from: &bool) -> i32 {
            if *from { 1i32 } else { 0i32 }
        }
    }

    impl ScalarConstructFrom<i32> for i32 {
        fn construct_from(from: &i32) -> i32 {
            *from
        }
    }

    impl ScalarConstructFrom<u32> for i32 {
        fn construct_from(from: &u32) -> i32 {
            *from as i32
        }
    }

    impl ScalarConstructFrom<f32> for i32 {
        fn construct_from(from: &f32) -> i32 {
            *from as i32
        }
    }

    impl ScalarConstructFrom<f64> for i32 {
        fn construct_from(from: &f64) -> i32 {
            *from as i32
        }
    }

    impl ScalarConstructFrom<bool> for u32 {
        fn construct_from(from: &bool) -> u32 {
            if *from { 1u32 } else { 0u32 }
        }
    }

    impl ScalarConstructFrom<i32> for u32 {
        fn construct_from(from: &i32) -> u32 {
            *from as u32
        }
    }

    impl ScalarConstructFrom<u32> for u32 {
        fn construct_from(from: &u32) -> u32 {
            *from
        }
    }

    impl ScalarConstructFrom<f32> for u32 {
        fn construct_from(from: &f32) -> u32 {
            *from as u32
        }
    }

    impl ScalarConstructFrom<f64> for u32 {
        fn construct_from(from: &f64) -> u32 {
            *from as u32
        }
    }

    impl ScalarConstructFrom<bool> for f32 {
        fn construct_from(from: &bool) -> f32 {
            if *from { 1f32 } else { 0f32 }
        }
    }

    impl ScalarConstructFrom<i32> for f32 {
        fn construct_from(from: &i32) -> f32 {
            *from as f32
        }
    }

    impl ScalarConstructFrom<u32> for f32 {
        fn construct_from(from: &u32) -> f32 {
            *from as f32
        }
    }

    impl ScalarConstructFrom<f32> for f32 {
        fn construct_from(from: &f32) -> f32 {
            *from
        }
    }

    impl ScalarConstructFrom<f64> for f32 {
        fn construct_from(from: &f64) -> f32 {
            *from as f32
        }
    }

    impl ScalarConstructFrom<bool> for f64 {
        fn construct_from(from: &bool) -> f64 {
            if *from { 1f64 } else { 0f64 }
        }
    }

    impl ScalarConstructFrom<i32> for f64 {
        fn construct_from(from: &i32) -> f64 {
            *from as f64
        }
    }

    impl ScalarConstructFrom<u32> for f64 {
        fn construct_from(from: &u32) -> f64 {
            *from as f64
        }
    }

    impl ScalarConstructFrom<f32> for f64 {
        fn construct_from(from: &f32) -> f64 {
            *from as f64
        }
    }

    impl ScalarConstructFrom<f64> for f64 {
        fn construct_from(from: &f64) -> f64 {
            *from
        }
    }

    fn add_sv_binop_components<T, F>(mut func: ConstEvalFunctionBuilder, f: F) -> ConstEvalFunctionBuilder where F: Fn(T, T) -> T + Clone + Send + Sync + 'static, T: ConstParameter + Scalar, ConstSVVal<T>: ConstParameter {
        let fc = f.clone();
        func = func.add_overload_2(move |a: ConstSVVal<T>, b: T| Some(a.map(|v| fc(v, b.clone()))));
        let fc = f.clone();
        func = func.add_overload_2(move |a: T, b: ConstSVVal<T>| Some(b.map(|v| fc(a.clone(), v))));
        let fc = f.clone();
        func.add_overload_2(move |a: ConstSVVal<T>, b: ConstSVVal<T>| a.zip_map(&b, &fc))
    }

    fn add_i32_binop_components<F>(func: ConstEvalFunctionBuilder, f: F) -> ConstEvalFunctionBuilder where F: Fn(i32, i32) -> i32 + Clone + Send + Sync + 'static {
        add_sv_binop_components(func, f)
    }

    fn add_u32_binop_components<F>(func: ConstEvalFunctionBuilder, f: F) -> ConstEvalFunctionBuilder where F: Fn(u32, u32) -> u32 + Clone + Send + Sync + 'static {
        add_sv_binop_components(func, f)
    }

    fn add_svm_binop_components<T, F>(mut func: ConstEvalFunctionBuilder, f: F) -> ConstEvalFunctionBuilder where F: Fn(T, T) -> T + Clone + Send + Sync + 'static, T: ConstParameter + Scalar, ConstSVVal<T>: ConstParameter, ConstMVal<T>: ConstParameter {
        func = add_sv_binop_components(func, f.clone());
        let fc = f.clone();
        func = func.add_overload_2(move |a: ConstMVal<T>, b: T| Some(a.map(|v| fc(v, b.clone()))));
        let fc = f.clone();
        func = func.add_overload_2(move |a: T, b: ConstMVal<T>| Some(b.map(|v| fc(v, a.clone()))));
        let fc = f.clone();
        func.add_overload_2(move |a: ConstMVal<T>, b: ConstMVal<T>| a.zip_map(&b, &fc))
    }

    fn add_f32_binop_components<F>(func: ConstEvalFunctionBuilder, f: F) -> ConstEvalFunctionBuilder where F: Fn(f32, f32) -> f32 + Clone + Send + Sync + 'static {
        add_svm_binop_components(func, f)
    }

    fn add_f64_binop_components<F>(func: ConstEvalFunctionBuilder, f: F) -> ConstEvalFunctionBuilder where F: Fn(f64, f64) -> f64 + Clone + Send + Sync + 'static {
        add_svm_binop_components(func, f)
    }

    lazy_static! {
        pub static ref OP_UNARY_ADD: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_1(|v: ConstSVVal<i32>| Some(v))
                .add_overload_1(|v: ConstSVVal<u32>| Some(v))
                .add_overload_1(|v: ConstSVMVal<f32>| Some(v))
                .add_overload_1(|v: ConstSVMVal<f64>| Some(v))
                .build()
        };
        pub static ref OP_UNARY_MINUS: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_1(|v: ConstSVVal<i32>| Some(v.map(i32::wrapping_neg)))
                .add_overload_1(|v: ConstSVVal<u32>| Some(v.map(u32::wrapping_neg)))
                .add_overload_1(|v: ConstSVMVal<f32>| Some(v.map(f32::neg)))
                .add_overload_1(|v: ConstSVMVal<f64>| Some(v.map(f64::neg)))
                .build()
        };
        pub static ref OP_UNARY_NOT: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_1(|v: bool| Some(!v))
                .build()
        };
        pub static ref OP_UNARY_COMPLEMENT: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_1(|v: ConstSVVal<i32>| Some(v.map(i32::not)))
                .add_overload_1(|v: ConstSVVal<u32>| Some(v.map(u32::not)))
                .build()
        };
        pub static ref OP_BINARY_OR: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: bool, b: bool| Some(a || b))
                .build()
        };
        pub static ref OP_BINARY_XOR: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: bool, b: bool| Some(a != b))
                .build()
        };
        pub static ref OP_BINARY_AND: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: bool, b: bool| Some(a && b))
                .build()
        };
        pub static ref OP_BINARY_BIT_OR: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, i32::bitor);
            f = add_u32_binop_components(f, u32::bitor);
            f.build()
        };
        pub static ref OP_BINARY_BIT_XOR: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, i32::bitxor);
            f = add_u32_binop_components(f, u32::bitxor);
            f.build()
        };
        pub static ref OP_BINARY_BIT_AND: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, i32::bitand);
            f = add_u32_binop_components(f, u32::bitand);
            f.build()
        };
        pub static ref OP_BINARY_EQUAL: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: ConstSVVal<bool>, b: ConstSVVal<bool>| Some(a.zip_map(&b, |a, b| a == b)?.fold(true, bool::bitand)))
                .add_overload_2(|a: ConstSVVal<i32>, b: ConstSVVal<i32>| Some(a.zip_map(&b, |a, b| a == b)?.fold(true, bool::bitand)))
                .add_overload_2(|a: ConstSVVal<u32>, b: ConstSVVal<u32>| Some(a.zip_map(&b, |a, b| a == b)?.fold(true, bool::bitand)))
                .add_overload_2(|a: ConstSVMVal<f32>, b: ConstSVMVal<f32>| Some(a.zip_map(&b, |a, b| a == b)?.fold(true, bool::bitand)))
                .add_overload_2(|a: ConstSVMVal<f64>, b: ConstSVMVal<f64>| Some(a.zip_map(&b, |a, b| a == b)?.fold(true, bool::bitand)))
                .build()
        };
        pub static ref OP_BINARY_LT: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: i32, b: i32| Some(a < b))
                .add_overload_2(|a: u32, b: u32| Some(a < b))
                .add_overload_2(|a: f32, b: f32| Some(a < b))
                .add_overload_2(|a: f64, b: f64| Some(a < b))
                .build()
        };
        pub static ref OP_BINARY_GT: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: i32, b: i32| Some(a > b))
                .add_overload_2(|a: u32, b: u32| Some(a > b))
                .add_overload_2(|a: f32, b: f32| Some(a > b))
                .add_overload_2(|a: f64, b: f64| Some(a > b))
                .build()
        };
        pub static ref OP_BINARY_LTE: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: i32, b: i32| Some(a <= b))
                .add_overload_2(|a: u32, b: u32| Some(a <= b))
                .add_overload_2(|a: f32, b: f32| Some(a <= b))
                .add_overload_2(|a: f64, b: f64| Some(a <= b))
                .build()
        };
        pub static ref OP_BINARY_GTE: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: i32, b: i32| Some(a >= b))
                .add_overload_2(|a: u32, b: u32| Some(a >= b))
                .add_overload_2(|a: f32, b: f32| Some(a >= b))
                .add_overload_2(|a: f64, b: f64| Some(a >= b))
                .build()
        };
        pub static ref OP_BINARY_LSHIFT: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: ConstSVVal<i32>, b: i32| Some(a.map(|v| v << b)))
                .add_overload_2(|a: ConstSVVal<i32>, b: u32| Some(a.map(|v| v << b)))
                .add_overload_2(|a: ConstSVVal<i32>, b: ConstSVVal<i32>| a.zip_map(&b, |a, b| a << b))
                .add_overload_2(|a: ConstSVVal<i32>, b: ConstSVVal<u32>| a.zip_map(&b, |a, b| a << b))
                .add_overload_2(|a: ConstSVVal<u32>, b: i32| Some(a.map(|v| v << b)))
                .add_overload_2(|a: ConstSVVal<u32>, b: u32| Some(a.map(|v| v << b)))
                .add_overload_2(|a: ConstSVVal<u32>, b: ConstSVVal<i32>| a.zip_map(&b, |a, b| a << b))
                .add_overload_2(|a: ConstSVVal<u32>, b: ConstSVVal<u32>| a.zip_map(&b, |a, b| a << b))
                .build()
        };
        pub static ref OP_BINARY_RSHIFT: ConstEvalFunction = {
            ConstEvalFunctionBuilder::new()
                .add_overload_2(|a: ConstSVVal<i32>, b: i32| Some(a.map(|v| v >> b)))
                .add_overload_2(|a: ConstSVVal<i32>, b: u32| Some(a.map(|v| v >> b)))
                .add_overload_2(|a: ConstSVVal<i32>, b: ConstSVVal<i32>| a.zip_map(&b, |a, b| a >> b))
                .add_overload_2(|a: ConstSVVal<i32>, b: ConstSVVal<u32>| a.zip_map(&b, |a, b| a >> b))
                .add_overload_2(|a: ConstSVVal<u32>, b: i32| Some(a.map(|v| v >> b)))
                .add_overload_2(|a: ConstSVVal<u32>, b: u32| Some(a.map(|v| v >> b)))
                .add_overload_2(|a: ConstSVVal<u32>, b: ConstSVVal<i32>| a.zip_map(&b, |a, b| a >> b))
                .add_overload_2(|a: ConstSVVal<u32>, b: ConstSVVal<u32>| a.zip_map(&b, |a, b| a >> b))
                .build()
        };
        pub static ref OP_BINARY_ADD: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, |a, b| a + b);
            f = add_u32_binop_components(f, |a, b| a + b);
            f = add_f32_binop_components(f, |a, b| a + b);
            f = add_f64_binop_components(f, |a, b| a + b);
            f.build()
        };
        pub static ref OP_BINARY_SUB: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, |a, b| a - b);
            f = add_u32_binop_components(f, |a, b| a - b);
            f = add_f32_binop_components(f, |a, b| a - b);
            f = add_f64_binop_components(f, |a, b| a - b);
            f.build()
        };
        pub static ref OP_BINARY_MULT: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, |a, b| a * b);
            f = add_u32_binop_components(f, |a, b| a * b);
            f = add_sv_binop_components(f, |a: f32, b: f32| a * b);
            f = add_sv_binop_components(f, |a: f64, b: f64| a * b);
            f.add_overload_2(|a: Vector2<f32>, b: Matrix2<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector2<f32>, b: Matrix2x3<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector2<f32>, b: Matrix2x4<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector3<f32>, b: Matrix3x2<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector3<f32>, b: Matrix3<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector3<f32>, b: Matrix3x4<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector4<f32>, b: Matrix4x2<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector4<f32>, b: Matrix4x3<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector4<f32>, b: Matrix4<f32>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Matrix2<f32>, b: Vector2<f32>| Some(a * b))
                .add_overload_2(|a: Matrix3x2<f32>, b: Vector2<f32>| Some(a * b))
                .add_overload_2(|a: Matrix4x2<f32>, b: Vector2<f32>| Some(a * b))
                .add_overload_2(|a: Matrix2x3<f32>, b: Vector3<f32>| Some(a * b))
                .add_overload_2(|a: Matrix3<f32>, b: Vector3<f32>| Some(a * b))
                .add_overload_2(|a: Matrix4x3<f32>, b: Vector3<f32>| Some(a * b))
                .add_overload_2(|a: Matrix2x4<f32>, b: Vector4<f32>| Some(a * b))
                .add_overload_2(|a: Matrix3x4<f32>, b: Vector4<f32>| Some(a * b))
                .add_overload_2(|a: Matrix4<f32>, b: Vector4<f32>| Some(a * b))
                .add_overload_2(|a: Vector2<f64>, b: Matrix2<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector2<f64>, b: Matrix2x3<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector2<f64>, b: Matrix2x4<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector3<f64>, b: Matrix3x2<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector3<f64>, b: Matrix3<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector3<f64>, b: Matrix3x4<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector4<f64>, b: Matrix4x2<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector4<f64>, b: Matrix4x3<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Vector4<f64>, b: Matrix4<f64>| Some((a.transpose() * b).transpose()))
                .add_overload_2(|a: Matrix2<f64>, b: Vector2<f64>| Some(a * b))
                .add_overload_2(|a: Matrix3x2<f64>, b: Vector2<f64>| Some(a * b))
                .add_overload_2(|a: Matrix4x2<f64>, b: Vector2<f64>| Some(a * b))
                .add_overload_2(|a: Matrix2x3<f64>, b: Vector3<f64>| Some(a * b))
                .add_overload_2(|a: Matrix3<f64>, b: Vector3<f64>| Some(a * b))
                .add_overload_2(|a: Matrix4x3<f64>, b: Vector3<f64>| Some(a * b))
                .add_overload_2(|a: Matrix2x4<f64>, b: Vector4<f64>| Some(a * b))
                .add_overload_2(|a: Matrix3x4<f64>, b: Vector4<f64>| Some(a * b))
                .add_overload_2(|a: Matrix4<f64>, b: Vector4<f64>| Some(a * b))
                .build()
        };
        pub static ref OP_BINARY_DIV: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, |a, b| a / b);
            f = add_u32_binop_components(f, |a, b| a / b);
            f = add_f32_binop_components(f, |a, b| a / b);
            f = add_f64_binop_components(f, |a, b| a / b);
            f.build()
        };
        pub static ref OP_BINARY_MOD: ConstEvalFunction = {
            let mut f = ConstEvalFunctionBuilder::new();
            f = add_i32_binop_components(f, |a, b| a % b);
            f = add_u32_binop_components(f, |a, b| a % b);
            f.build()
        };
    }

    fn add_scalar_constructor<T>(f: ConstEvalFunctionBuilder) -> ConstEvalFunctionBuilder where T: Scalar + ConstParameter + ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64> {
        f.add_overload_1(|v: ConstSVVal<bool>| Some(T::construct_from(v.column_iter().next().unwrap())))
            .add_overload_1(|v: ConstSVVal<i32>| Some(T::construct_from(v.column_iter().next().unwrap())))
            .add_overload_1(|v: ConstSVVal<u32>| Some(T::construct_from(v.column_iter().next().unwrap())))
            .add_overload_1(|v: ConstSVVal<f32>| Some(T::construct_from(v.column_iter().next().unwrap())))
            .add_overload_1(|v: ConstSVVal<f64>| Some(T::construct_from(v.column_iter().next().unwrap())))
    }

    enum ScalarIterWrapper<'a, T: ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64>> {
        Bool(std::slice::Iter<'a, bool>, PhantomData<T>),
        Int(std::slice::Iter<'a, i32>),
        UInt(std::slice::Iter<'a, u32>),
        Float(std::slice::Iter<'a, f32>),
        Double(std::slice::Iter<'a, f64>),
    }

    impl<'a, T> ScalarIterWrapper<'a, T> where T: ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64> {
        fn from_base_val(val: &'a ConstBaseVal) -> Self {
            match val {
                ConstBaseVal::Bool(v) => Self::Bool(v.column_iter(), PhantomData),
                ConstBaseVal::Int(v) => Self::Int(v.column_iter()),
                ConstBaseVal::UInt(v) => Self::UInt(v.column_iter()),
                ConstBaseVal::Float(v) => Self::Float(v.column_iter()),
                ConstBaseVal::Double(v) => Self::Double(v.column_iter()),
            }
        }
    }

    impl<'a, T> Iterator for ScalarIterWrapper<'a, T> where T: ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64> {
        type Item = T;

        fn next(&mut self) -> Option<Self::Item> {
            match self {
                ScalarIterWrapper::Bool(i, _) => i.next().map(T::construct_from),
                ScalarIterWrapper::Int(i) => i.next().map(T::construct_from),
                ScalarIterWrapper::UInt(i) => i.next().map(T::construct_from),
                ScalarIterWrapper::Float(i) => i.next().map(T::construct_from),
                ScalarIterWrapper::Double(i) => i.next().map(T::construct_from),
            }
        }
    }

    struct ValIterator<'a, 'b, T: ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64>> {
        params: &'a[&'b ConstBaseVal],
        current_param: usize,
        current_iter: Option<ScalarIterWrapper<'b, T>>,
    }

    impl<'a, 'b, T> ValIterator<'a, 'b, T> where T: ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64> {
        fn new(params: &'a[&'b ConstBaseVal]) -> Self {
            let current_iter = if params.len() != 0 {
                Some(ScalarIterWrapper::from_base_val(params[0]))
            } else {
                None
            };

            Self {
                params,
                current_param: 0,
                current_iter,
            }
        }
    }

    impl<'a, 'b, T> Iterator for ValIterator<'a, 'b, T> where T: ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64> {
        type Item = T;

        fn next(&mut self) -> Option<Self::Item> {
            loop {
                let iter = self.current_iter.as_mut()?;
                if let Some(next) = iter.next() {
                    return Some(next);
                } else {
                    self.current_param += 1;
                    if self.current_param >= self.params.len() {
                        self.current_iter = None;
                        return None;
                    } else {
                        self.current_iter = Some(ScalarIterWrapper::from_base_val(self.params[self.current_param]));
                    }
                }
            }
        }
    }

    type AVector<const R: usize, T> = Matrix<T, Const<R>, U1, ArrayStorage<T, R, 1>>;
    fn add_vec_constructor<const R: usize, T>(f: ConstEvalFunctionBuilder) -> ConstEvalFunctionBuilder where T: Scalar + ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64>, AVector<R, T>: ConstParameter {
        f.add_generic(|params| {
            if params.len() == 0 {
                return None;
            }
            if params.len() == 1 {
                if params[0].get_shape() == BaseTypeShape::Scalar {
                    return Some(AVector::<R, T>::from_element(ValIterator::new(params).next().unwrap()).into_const_base_val());
                }
            }

            if ValIterator::<T>::new(params).count() >= R {
                Some(AVector::<R, T>::from_iterator(ValIterator::new(params)).into_const_base_val())
            } else {
                None
            }
        })
    }

    fn copy_to_mat<const R1: usize, const C1: usize, const R2: usize, const C2: usize, T: Scalar>(from: &AMatrix<R1, C1, T>, to: &mut AMatrix<R2, C2, T>) {
        for r in 0..std::cmp::min(R1, R2) {
            for c in 0..std::cmp::min(C1, C2) {
                to[(r, c)] = from[(r, c)].clone();
            }
        }
    }

    type AMatrix<const R: usize, const C: usize, T> = Matrix<T, Const<R>, Const<C>, ArrayStorage<T, R, C>>;
    fn add_mat_constructor<const R: usize, const C: usize, T>(f: ConstEvalFunctionBuilder) -> ConstEvalFunctionBuilder where T: Scalar + Zero + One + ScalarConstructFrom<bool> + ScalarConstructFrom<i32> + ScalarConstructFrom<u32> + ScalarConstructFrom<f32> + ScalarConstructFrom<f64>, AMatrix<R, C, T>: ConstParameter {
        f.add_generic(|params| {
            if params.len() == 0 {
                return None;
            }
            if params.len() == 1 {
                if params[0].get_shape() == BaseTypeShape::Scalar {
                    return Some(AMatrix::<R, C, T>::from_diagonal_element(ValIterator::new(params).next().unwrap()).into_const_base_val());
                } else {
                    let converted = match params[0] {
                        ConstBaseVal::Float(ConstSVMVal::Matrix(v)) => Some(v.map(|v| T::construct_from(&v))),
                        ConstBaseVal::Double(ConstSVMVal::Matrix(v)) => Some(v.map(|v| T::construct_from(&v))),
                        _ => None,
                    };
                    if let Some(converted) = converted {
                        let mut result = AMatrix::<R, C, T>::identity();
                        match converted {
                            ConstMVal::Mat2(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat23(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat24(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat32(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat3(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat34(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat42(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat43(v) => copy_to_mat(&v, &mut result),
                            ConstMVal::Mat4(v) => copy_to_mat(&v, &mut result),
                        };
                        return Some(result.into_const_base_val());
                    }
                }
            }

            if ValIterator::<T>::new(params).count() >= R * C {
                Some(AMatrix::<R, C, T>::from_iterator(ValIterator::new(params)).into_const_base_val())
            } else {
                None
            }
        })
    }

    /// Registers all type constructors as const functions
    pub fn register_constructor_const_functions<F: FnMut(Identifier, ConstEvalFunction)>(mut f: F) {
        f(Identifier::new("bool").unwrap(), add_scalar_constructor::<bool>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("int").unwrap(), add_scalar_constructor::<i32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("uint").unwrap(), add_scalar_constructor::<u32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("float").unwrap(), add_scalar_constructor::<f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("double").unwrap(), add_scalar_constructor::<f64>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("bvec2").unwrap(), add_vec_constructor::<2, bool>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("ivec2").unwrap(), add_vec_constructor::<2, i32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("uvec2").unwrap(), add_vec_constructor::<2, u32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("vec2").unwrap(), add_vec_constructor::<2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dvec2").unwrap(), add_vec_constructor::<2, f64>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("bvec3").unwrap(), add_vec_constructor::<3, bool>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("ivec3").unwrap(), add_vec_constructor::<3, i32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("uvec3").unwrap(), add_vec_constructor::<3, u32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("vec3").unwrap(), add_vec_constructor::<3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dvec3").unwrap(), add_vec_constructor::<3, f64>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("bvec4").unwrap(), add_vec_constructor::<4, bool>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("ivec4").unwrap(), add_vec_constructor::<4, i32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("uvec4").unwrap(), add_vec_constructor::<4, u32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("vec4").unwrap(), add_vec_constructor::<4, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dvec4").unwrap(), add_vec_constructor::<4, f64>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat2").unwrap(), add_mat_constructor::<2, 2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat23").unwrap(), add_mat_constructor::<2, 3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat24").unwrap(), add_mat_constructor::<2, 4, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat32").unwrap(), add_mat_constructor::<3, 2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat3").unwrap(), add_mat_constructor::<3, 3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat34").unwrap(), add_mat_constructor::<3, 4, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat42").unwrap(), add_mat_constructor::<4, 2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat43").unwrap(), add_mat_constructor::<4, 3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("mat4").unwrap(), add_mat_constructor::<4, 4, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat2").unwrap(), add_mat_constructor::<2, 2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat23").unwrap(), add_mat_constructor::<2, 3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat24").unwrap(), add_mat_constructor::<2, 4, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat32").unwrap(), add_mat_constructor::<3, 2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat3").unwrap(), add_mat_constructor::<3, 3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat34").unwrap(), add_mat_constructor::<3, 4, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat42").unwrap(), add_mat_constructor::<4, 2, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat43").unwrap(), add_mat_constructor::<4, 3, f32>(ConstEvalFunctionBuilder::new()).build());
        f(Identifier::new("dmat4").unwrap(), add_mat_constructor::<4, 4, f32>(ConstEvalFunctionBuilder::new()).build());
    }

    pub fn register_builtin_const_functions<F: FnMut(Identifier, ConstEvalFunction)>(f: F) {
        register_constructor_const_functions(f);
    }

    lazy_static! {
        pub static ref BUILTIN_CONST_FUNCTIONS: HashMap<String, ConstEvalFunction> = {
            let mut map = HashMap::new();
            register_builtin_const_functions(|i, f| { map.insert(i.0, f); });
            map
        };
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const BASE_TYPE_VALUES: &[ParameterBaseType] = &[ParameterBaseType::Bool, ParameterBaseType::Int, ParameterBaseType::UInt, ParameterBaseType::Float, ParameterBaseType::Double];
        const SHAPE_VALUES: &[ParameterShape] = &[ParameterShape::Scalar, ParameterShape::Vec2, ParameterShape::Vec3, ParameterShape::Vec4, ParameterShape::Mat2, ParameterShape::Mat23, ParameterShape::Mat24, ParameterShape::Mat32, ParameterShape::Mat3, ParameterShape::Mat34, ParameterShape::Mat42, ParameterShape::Mat43, ParameterShape::Mat4, ParameterShape::GenericM, ParameterShape::GenericSV, ParameterShape::GenericSVM];

        #[test]
        fn base_type_order_samples() {
            assert_eq!(ParameterBaseType::Bool.cast_cmp(&ParameterBaseType::Bool), Some(Ordering::Equal));
            assert_eq!(ParameterBaseType::Bool.cast_cmp(&ParameterBaseType::Float), None);
            assert_eq!(ParameterBaseType::Int.cast_cmp(&ParameterBaseType::UInt), Some(Ordering::Less));
            assert_eq!(ParameterBaseType::Double.cast_cmp(&ParameterBaseType::UInt), Some(Ordering::Greater));
        }

        #[test]
        fn base_type_order_consistency() {
            for a in BASE_TYPE_VALUES {
                for b in BASE_TYPE_VALUES {
                    let expected_bca = a.cast_cmp(b).map(Ordering::reverse);
                    assert_eq!(b.cast_cmp(a), expected_bca);
                }
            }
        }

        #[test]
        fn parameter_type_order_consistency() {
            let mut types = Vec::with_capacity(BASE_TYPE_VALUES.len() * SHAPE_VALUES.len());
            for base_type in BASE_TYPE_VALUES {
                for size in SHAPE_VALUES {
                    types.push(ParameterType::new(*base_type, *size));
                }
            }
            for a in &types {
                for b in &types {
                    let expected_bca = a.cast_cmp(b).reverse();
                    assert_eq!(b.cast_cmp(a), expected_bca);
                }
            }
        }

        #[test]
        fn parameter_type_order_samples() {
            let assert = |ty1: ParameterType, ty2: ParameterType| {
                // We only need less since we exhaustively verify consistency in the parameter_type_order_consistency test
                assert_eq!(ty1.cast_cmp(&ty2), Ordering::Less);
            };

            type T = ParameterType;
            type B = ParameterBaseType;
            type S = ParameterShape;
            assert(T::new(B::Int, S::Scalar), T::new(B::UInt, S::Scalar));
            assert(T::new(B::Int, S::Scalar), T::new(B::Float, S::Scalar));
            assert(T::new(B::Int, S::Scalar), T::new(B::Double, S::Scalar));

            assert(T::new(B::Bool, S::Mat3), T::new(B::Bool, S::GenericSVM));
            assert(T::new(B::UInt, S::Mat3), T::new(B::UInt, S::GenericSVM));
            assert(T::new(B::Double, S::Mat3), T::new(B::Double, S::GenericSVM));
        }

        #[test]
        fn const_eval_function_instance_compatibility() {
            type S = BaseTypeShape;
            type T = ParameterBaseType;

            let func = ConstEvalFunctionInstance::from_fn_0(|| 5u32);
            assert!(func.compatible_with(&[]));
            assert!(!func.compatible_with(&[(S::Scalar, T::Bool)]));
            assert!(!func.compatible_with(&[(S::Mat4, T::Float), (S::Mat4, T::Float), (S::Mat4, T::Float), (S::Mat4, T::Float), (S::Mat4, T::Float)]));

            let func = ConstEvalFunctionInstance::from_fn_1(|v: Matrix4<f64>| Some(v));
            assert!(!func.compatible_with(&[]));
            assert!(func.compatible_with(&[(S::Mat4, T::Float)]));
            assert!(func.compatible_with(&[(S::Mat4, T::Double)]));
            assert!(!func.compatible_with(&[(S::Mat4, T::Float), (S::Mat4, T::Float)]));
        }

        #[test]
        fn op_unary_add() {
            let v = ConstBaseVal::from(true);
            assert_eq!(OP_UNARY_ADD.eval(&[&v]), None);
            let v = ConstBaseVal::from(-3i32);
            assert_eq!(OP_UNARY_ADD.eval(&[&v]), Some(v));
            let v = ConstBaseVal::from(Vector2::from_element(4u32));
            assert_eq!(OP_UNARY_ADD.eval(&[&v]), Some(v));
            let v = ConstBaseVal::from(Matrix4::from_diagonal_element(-34f32));
            assert_eq!(OP_UNARY_ADD.eval(&[&v]), Some(v));
            let v = ConstBaseVal::from(Vector4::new(-9f64, 0f64, 3.78342979823f64, 1f64));
            assert_eq!(OP_UNARY_ADD.eval(&[&v]), Some(v));
        }

        #[test]
        fn op_unary_minus() {
            assert_eq!(OP_UNARY_MINUS.eval(&[&ConstBaseVal::from(true)]), None);
            assert_eq!(OP_UNARY_MINUS.eval(&[&ConstBaseVal::from(-3i32)]), Some(ConstBaseVal::from(3i32)));
            assert_eq!(OP_UNARY_MINUS.eval(&[&ConstBaseVal::from(Vector2::from_element(4u32))]), Some(ConstBaseVal::from(Vector2::from_element(4294967292u32))));
            assert_eq!(OP_UNARY_MINUS.eval(&[&ConstBaseVal::from(Matrix4::from_diagonal_element(-34f32))]), Some(ConstBaseVal::from(Matrix4::from_diagonal_element(34f32))));
            assert_eq!(OP_UNARY_MINUS.eval(&[&ConstBaseVal::from(Vector4::new(-9f64, 0f64, 3.78342979823f64, 1f64))]), Some(ConstBaseVal::from(Vector4::new(9f64, 0f64, -3.78342979823f64, -1f64))));
        }

        #[test]
        fn op_unary_not() {
            assert_eq!(OP_UNARY_NOT.eval(&[&ConstBaseVal::from(true)]), Some(ConstBaseVal::from(false)));
            assert_eq!(OP_UNARY_NOT.eval(&[&ConstBaseVal::from(Vector3::from_element(false))]), None);
            assert_eq!(OP_UNARY_NOT.eval(&[&ConstBaseVal::from(-3i32)]), None);
            assert_eq!(OP_UNARY_NOT.eval(&[&ConstBaseVal::from(Vector2::from_element(4u32))]), None);
            assert_eq!(OP_UNARY_NOT.eval(&[&ConstBaseVal::from(Matrix4::from_diagonal_element(-34f32))]), None);
            assert_eq!(OP_UNARY_NOT.eval(&[&ConstBaseVal::from(Vector4::new(-9f64, 0f64, 3.78342979823f64, 1f64))]), None);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct EmptyConstLookup();

    impl ConstLookup for EmptyConstLookup {
        fn lookup_const(&self, ident: &Identifier) -> Option<&ConstVal> {
            todo!()
        }
    }

    #[test]
    fn const_propagate_expr_base_construct() {
        const TEST_DATA: &[(Expr, Result<ConstValOrExpr, ConstEvalError>)] = &[
            (Expr::IntConst(-43i32), Ok(ConstBaseVal::new_int(-43i32).into())),

        ];
        for (case, expected) in TEST_DATA {
            assert_eq!(const_propagate_expr(case, &EmptyConstLookup(), &super::function::BUILTIN_CONST_FUNCTIONS), expected);
        }
    }
}