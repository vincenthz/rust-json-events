use std::mem;

#[allow(dead_code)]
pub struct Config {
	buffer_initial_size: usize,
	max_nesting: usize,
	max_data: usize,
	allow_c_comments: bool,
	allow_yaml_comments: bool
}

type JResult<T> = Result<T, JError>;
type JResult0 = JResult<()>;

/// JSON event
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Jev {
    ArrayStart,
    ObjectStart,
    ArrayEnd,
    ObjectEnd,
    Int,
    Float,
    String,
    Key,
    False,
    True,
    Null,
}

#[allow(non_camel_case_types)]
pub enum JError {
	/* SUCCESS = 0 */
	/* running out of memory */
	NO_MEMORY = 1,
	/* character < 32, except space newline tab */
	BAD_CHAR,
	/* trying to pop more object/array than pushed on the stack */
	POP_EMPTY,
	/* trying to pop wrong type of mode. popping array in object mode, vice versa */
	POP_UNEXPECTED_MODE,
	/* reach nesting limit on stack */
	NESTING_LIMIT,
	/* reach data limit on buffer */
	DATA_LIMIT,
	/* comment are not allowed with current configuration */
	COMMENT_NOT_ALLOWED,
	/* unexpected char in the current parser context */
	UNEXPECTED_CHAR,
	/* unicode low surrogate missing after high surrogate */
	UNICODE_MISSING_LOW_SURROGATE,
	/* unicode low surrogate missing without previous high surrogate */
	UNICODE_UNEXPECTED_LOW_SURROGATE,
	/* found a comma not in structure (array/object) */
	COMMA_OUT_OF_STRUCTURE,
	/* callback returns error */
	CALLBACK,
	/* utf8 stream is invalid */
	UTF8,
}

#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
enum C {
	Space, // space
	Nl,    // newline
	White, // tab, R
	Lcurb, Rcurb, // object opening/closing
	Lsqrb, Rsqrb, // array opening/closing
	// syntax symbols
	Colon,
	Comma,
	Quote, // "
	Backs, // \
	Slash, // /
	Plus,
	Minus,
	Dot,
	Zero, Digit, // digits
	a, b, c, d, e, f, l, n, r, s, t, u, // nocaps letters
	Abcdf, E, // caps letters
	Other, // all other
	Star, // star in C style comment
	Hash, // # for YAML comment
	Error = 0xfe,
}

/// define all states and actions that will be taken on each transition.
///
/// states are defined first because of the fact they are use as index in the
/// transitions table. they usually contains either a number or a prefix _
/// for simple state like string, object, value ...
///
/// actions are defined starting from 0x80. state error is defined as 0xff
///
#[repr(u8)]
#[derive(Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
enum S {
	GO, // start
	OK, // ok
	_O, // object
	_K, // key
	CO, // colon
	_V, // value
	_A, // array
	_S, // string
	E0, // escape
	U1, U2, U3, U4, // unicode states
	M0, Z0, I0, // number states
	R1, R2, // real states (after-dot digits)
	X1, X2, X3, // exponant states
	T1, T2, T3, // true constant states
	F1, F2, F3, F4, // false constant states
	N1, N2, N3, // null constant states
	C1, C2, C3, // C-comment states
	Y1, // YAML-comment state
	D1, D2, // multi unicode states
    // the following are actions that need to be taken
	KS = 0x80, // key separator
	SP, // comma separator
	AB, // array begin
	AE, // array ending
	OB, // object begin
	OE, // object end
	CB, // C-comment begin
	YB, // YAML-comment begin
	CE, // YAML/C comment end
	FA, // false
	TR, // true
	NU, // null
	DE, // double detected by exponent
	DF, // double detected by .
	SE, // string end
	MX, // integer detected by minus
	ZX, // integer detected by zero
	IX, // integer detected by 1-9
	UC, // Unicode character read
    __ = 0xff
}

fn is_state_above_array(st: S) -> bool {
    let st_num : u8 = unsafe { mem::transmute(st) };
    let a_num  : u8 = unsafe { mem::transmute(S::_A) };
    st_num > a_num
}

const NR_CLASSES : usize = 34;
const NR_STATES : usize = 37;

/* map from character < 128 to classes. from 128 to 256 all C_OTHER */
const CHARACTER_CLASS : [C;128] = [
    // 0 to 31
	C::Error, C::Error, C::Error, C::Error,
    C::Error, C::Error, C::Error, C::Error,
	C::Error, C::White, C::Nl,    C::Error,
    C::Error, C::White, C::Error, C::Error,
	C::Error, C::Error, C::Error, C::Error,
    C::Error, C::Error, C::Error, C::Error,
	C::Error, C::Error, C::Error, C::Error,
    C::Error, C::Error, C::Error, C::Error,
    // 32 to 63
	C::Space, C::Other, C::Quote, C::Hash,
    C::Other, C::Other, C::Other, C::Other,
	C::Other, C::Other, C::Star,  C::Plus,
    C::Comma, C::Minus, C::Dot,   C::Slash,
	C::Zero,  C::Digit, C::Digit, C::Digit,
    C::Digit, C::Digit, C::Digit, C::Digit,
	C::Digit, C::Digit, C::Colon, C::Other,
    C::Other, C::Other, C::Other, C::Other,
    // 64 to 95
	C::Other, C::Abcdf, C::Abcdf, C::Abcdf,
    C::Abcdf, C::E,     C::Abcdf, C::Other,
	C::Other, C::Other, C::Other, C::Other,
    C::Other, C::Other, C::Other, C::Other,
	C::Other, C::Other, C::Other, C::Other,
    C::Other, C::Other, C::Other, C::Other,
	C::Other, C::Other, C::Other, C::Lsqrb,
    C::Backs, C::Rsqrb, C::Other, C::Other,
    // 96 to 127
	C::Other, C::a,     C::b,     C::c,
    C::d,     C::e,     C::f,     C::Other,
	C::Other, C::Other, C::Other, C::Other,
    C::l,     C::Other, C::n,     C::Other,
	C::Other, C::Other, C::r,     C::s,
    C::t,     C::u,     C::Other, C::Other,
	C::Other, C::Other, C::Other, C::Lcurb,
    C::Other, C::Rcurb, C::Other, C::Other
];


macro_rules! st {
    ($a0:ident,$a1:ident,$a2:ident,$a3:ident,$a4:ident,$a5:ident,$a6:ident,$a7:ident,$a8:ident,$a9:ident,$a10:ident,$a11:ident,$a12:ident,$a13:ident,$a14:ident,$a15:ident,$a16:ident,$a17:ident,$a18:ident,$a19:ident,$a20:ident,$a21:ident,$a22:ident,$a23:ident,$a24:ident,$a25:ident,$a26:ident,$a27:ident,$a28:ident,$a29:ident,$a30:ident,$a31:ident,$a32:ident,$a33:ident) =>

( [ S::$a0,S::$a1,S::$a2,S::$a3,S::$a4,S::$a5,S::$a6,S::$a7,S::$a8,S::$a9,S::$a10,S::$a11,S::$a12,S::$a13,S::$a14,S::$a15,S::$a16,S::$a17,S::$a18,S::$a19,S::$a20,S::$a21,S::$a22,S::$a23,S::$a24,S::$a25,S::$a26,S::$a27,S::$a28,S::$a29,S::$a30,S::$a31,S::$a32,S::$a33 ] )
}

const STATE_TRANS : [[S;NR_CLASSES];NR_STATES] = [
/*GO*/ st!(GO,GO,GO,OB,__,AB,__,__,__,__,__,CB,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,YB),
/*OK*/ st!(OK,OK,OK,__,OE,__,AE,__,SP,__,__,CB,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,YB),
/*_O*/ st!(_O,_O,_O,__,OE,__,__,__,__,_S,__,CB,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,YB),
/*_K*/ st!(_K,_K,_K,__,__,__,__,__,__,_S,__,CB,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,YB),
/*CO*/ st!(CO,CO,CO,__,__,__,__,KS,__,__,__,CB,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,YB),
/*_V*/ st!(_V,_V,_V,OB,__,AB,__,__,__,_S,__,CB,__,MX,__,ZX,IX,__,__,__,__,__,F1,__,N1,__,__,T1,__,__,__,__,__,YB),
/*_A*/ st!(_A,_A,_A,OB,__,AB,AE,__,__,_S,__,CB,__,MX,__,ZX,IX,__,__,__,__,__,F1,__,N1,__,__,T1,__,__,__,__,__,YB),
/****************************************************************************************************************/
/*_S*/ st!(_S,__,__,_S,_S,_S,_S,_S,_S,SE,E0,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S,_S),
/*E0*/ st!(__,__,__,__,__,__,__,__,__,_S,_S,_S,__,__,__,__,__,__,_S,__,__,__,_S,__,_S,_S,__,_S,U1,__,__,__,__,__),
/*U1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,U2,U2,U2,U2,U2,U2,U2,U2,__,__,__,__,__,__,U2,U2,__,__,__),
/*U2*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,U3,U3,U3,U3,U3,U3,U3,U3,__,__,__,__,__,__,U3,U3,__,__,__),
/*U3*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,U4,U4,U4,U4,U4,U4,U4,U4,__,__,__,__,__,__,U4,U4,__,__,__),
/*U4*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,UC,UC,UC,UC,UC,UC,UC,UC,__,__,__,__,__,__,UC,UC,__,__,__),
/****************************************************************************************************************/
/*M0*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,Z0,I0,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/*Z0*/ st!(OK,OK,OK,__,OE,__,AE,__,SP,__,__,CB,__,__,DF,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,YB),
/*I0*/ st!(OK,OK,OK,__,OE,__,AE,__,SP,__,__,CB,__,__,DF,I0,I0,__,__,__,__,DE,__,__,__,__,__,__,__,__,DE,__,__,YB),
/*R1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,R2,R2,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/*R2*/ st!(OK,OK,OK,__,OE,__,AE,__,SP,__,__,CB,__,__,__,R2,R2,__,__,__,__,X1,__,__,__,__,__,__,__,__,X1,__,__,YB),
/*X1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,X2,X2,__,X3,X3,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/*X2*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,X3,X3,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/*X3*/ st!(OK,OK,OK,__,OE,__,AE,__,SP,__,__,__,__,__,__,X3,X3,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/****************************************************************************************************************/
/*T1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,T2,__,__,__,__,__,__,__,__),
/*T2*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,T3,__,__,__,__,__),
/*T3*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,TR,__,__,__,__,__,__,__,__,__,__,__,__),
/*F1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,F2,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/*F2*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,F3,__,__,__,__,__,__,__,__,__,__),
/*F3*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,F4,__,__,__,__,__,__,__),
/*F4*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,FA,__,__,__,__,__,__,__,__,__,__,__,__),
/*N1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,N2,__,__,__,__,__),
/*N2*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,N3,__,__,__,__,__,__,__,__,__,__),
/*N3*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,NU,__,__,__,__,__,__,__,__,__,__),
/****************************************************************************************************************/
/*C1*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,C2,__),
/*C2*/ st!(C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C3,C2),
/*C3*/ st!(C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,CE,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C2,C3,C2),
/*Y1*/ st!(Y1,CE,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1,Y1),
/*D1*/ st!(__,__,__,__,__,__,__,__,__,__,D2,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__),
/*D2*/ st!(__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,U1,__,__,__,__,__),
];

/* map from (previous state+new character class) to the buffer policy. ignore=0/append=1/escape=2 */
const BUFFER_POLICY_TABLE : [[u8;NR_CLASSES];NR_STATES] = [
/*          white                                                                            ABCDF  other     */
/*      sp nl  |  {  }  [  ]  :  ,  "  \  /  +  -  .  0  19 a  b  c  d  e  f  l  n  r  s  t  u  |  E  |  *  # */
/*GO*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*OK*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*_O*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*_K*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*CO*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*_V*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*_A*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/**************************************************************************************************************/
/*_S*/ [ 1, 0, 0, 1, 1, 1, 1, 1, 1, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1 ],
/*E0*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 2, 2, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 2, 0, 2, 2, 0, 2, 0, 0, 0, 0, 0, 0 ],
/*U1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0 ],
/*U2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0 ],
/*U3*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0 ],
/*U4*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0 ],
/**************************************************************************************************************/
/*M0*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*Z0*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*I0*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0 ],
/*R1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*R2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0 ],
/*X1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*X2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*X3*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/**************************************************************************************************************/
/*T1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*T2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*T3*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*F1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*F2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*F3*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*F4*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*N1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*N2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*N3*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/**************************************************************************************************************/
/*C1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*C2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*C3*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*Y1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*D1*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
/*D2*/ [ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0 ],
    ];

const __ : u8 = 0xff;
const UTF8_HEADER_TABLE : [u8;256] =
[
/* 00 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 10 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 20 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 30 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 40 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 50 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 60 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 70 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 80 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 90 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* a0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* b0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* c0 */ 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
/* d0 */ 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
/* e0 */ 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2,
/* f0 */ 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5,__,__,
];

const UTF8_CONTINUATION_TABLE : [u8;256] =
[
/*__0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 10 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 20 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 30 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 40 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 50 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 60 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 70 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* 80 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* 90 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* a0 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* b0 */ 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
/* c0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* d0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* e0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
/* f0 */__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,__,
];

const HEXTABLE : [u32; 128] = [
	255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,
	255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,
	255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,
	  0,  1,  2,  3,  4,  5,  6,  7,  8,  9,255,255,255,255,255,255,
	255, 10, 11, 12, 13, 14, 15,255,255,255,255,255,255,255,255,255,
	255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,
	255, 10, 11, 12, 13, 14, 15,255,255,255,255,255,255,255,255,255,
	255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,255,
    ];

fn is_high_surrogate(uc: u32) -> bool { (uc & 0xfc00) == 0xd800 }
fn is_low_surrogate(uc: u32) -> bool { (uc & 0xfc00) == 0xdc00 }

#[repr(u8)]
#[derive(PartialEq)]
pub enum StackMode {
    Object,
    Array
}

pub type Callback = Fn(Jev, Option<&Vec<u8> >) -> Result<(), JError>;

pub struct Parser {
    config: Config,
    state: S,
    save_state: S,
    expecting_key: bool,
	utf8_multibyte_left: u8,
	unicode_multi: u32,
    stack: Vec<StackMode>,
    //stack_size: usize,
    jtype: Option<Jev>,
    buffer: Vec<u8>,
    buffer_size: usize
}

// initialize a parser structure taking a config,
pub fn init(config: Config) -> Parser {
    Parser { 
        config: config,
        state: S::GO,
        save_state: S::GO,
        expecting_key: false,
        utf8_multibyte_left: 0,
        unicode_multi: 0,
        stack: vec![],
        jtype: None,
        buffer: vec![],
        buffer_size: 2048,
    }
}

fn state_push(parser: &mut Parser, mode: StackMode) {
    parser.stack.push(mode);
}

fn state_pop(parser: &mut Parser, mode: StackMode) -> JResult0 {
    match parser.stack.pop() {
        None    => Err(JError::POP_EMPTY),
        Some(m) =>
	        if m == mode { Ok (()) } else { Err(JError::POP_UNEXPECTED_MODE) }
    }
}

fn buffer_push(parser: &mut Parser, c: u8) -> JResult0 {
	if parser.buffer.len() >= parser.buffer_size {
        Err(JError::DATA_LIMIT)
    } else {
        parser.buffer.push(c);
        Ok(())
    }
}

fn buffer_push_escape(parser: &mut Parser, next: u8) -> JResult0 {
	let c =
        match next {
            0x62 /* 'b' */  => 0x7,
            0x66 /* 'f' */  => 0xc,
            0x6e /* 'n' */  => 0x10,
            0x72 /* 'r' */  => 0x13,
            0x74 /* 't' */  => 0x9,
            0x22 /* '"' */  => 0x22,
            0x2f /* '/' */  => 0x2f,
            0x5c /* '\\' */ => 0x5c,
            _               => 0
        };
	buffer_push(parser, c)
}

fn do_callback_withbuf(parser: &mut Parser, cb: &Callback, ty: Jev) -> JResult0 {
    cb(ty, Some(&parser.buffer))
}

fn do_callback(_: &mut Parser, cb: &Callback, ty: Jev) -> JResult0 {
    cb(ty, None)
}

fn do_buffer(parser: &mut Parser, cb: &Callback) -> JResult0 {
    match parser.jtype {
        Some(jty) =>
            match jty {
                Jev::Key    => try!(do_callback_withbuf(parser, cb, jty)),
                Jev::String => try!(do_callback_withbuf(parser, cb, jty)),
                Jev::Float  => try!(do_callback_withbuf(parser, cb, jty)),
                Jev::Int    => try!(do_callback_withbuf(parser, cb, jty)),
                Jev::Null   => try!(do_callback_withbuf(parser, cb, jty)),
                Jev::True   => try!(do_callback_withbuf(parser, cb, jty)),
                Jev::False  => try!(do_callback_withbuf(parser, cb, jty)),
                _           => ()
            },
        _         => (),
    };
    parser.buffer.clear();
    Ok(())
}

fn update_simple(parser: &mut Parser, ty: Option<Jev>, nst: S) -> JResult0 {
    match nst {
        S::__ => (),
        _     => parser.state = nst
    };
    parser.jtype = ty;
    Ok(())
}

fn update_callbk<F>(parser: &mut Parser, cb: &Callback, ty: Option<Jev>, nst: S, dobuf: bool, per_ty_cb: F) -> JResult0
    where F : Fn(&mut Parser) -> JResult0 {
    if dobuf {
	    try!(do_buffer(parser, cb));
    }
    try!(per_ty_cb(parser));
    match nst {
        S::__ => (),
        _     => parser.state = nst
    };
    parser.jtype = ty;
    Ok(())
}

/* transform an unicode [0-9A-Fa-f]{4} sequence into a proper value */
fn decode_unicode_char(parser: &mut Parser) -> JResult0 {
    let offset = parser.buffer.len();
    let uval = HEXTABLE[parser.buffer[offset - 4] as usize] << 12
             | HEXTABLE[parser.buffer[offset - 3] as usize] << 8
             | HEXTABLE[parser.buffer[offset - 2] as usize] << 4
             | HEXTABLE[parser.buffer[offset - 1] as usize];

    parser.buffer.truncate(offset - 4);
    
	if parser.unicode_multi > 0 && uval < 0x80 {
        parser.buffer.push(uval as u8);
        return Ok(())
	}

    if parser.unicode_multi > 0 {
        if !is_low_surrogate(uval) {
            return Err(JError::UNICODE_MISSING_LOW_SURROGATE);
        }

        let uval = 0x10000 + ((parser.unicode_multi & 0x3ff) << 10) + (uval & 0x3ff);
        parser.buffer.push(((uval >> 18) | 0xf0) as u8);
        parser.buffer.push((((uval >> 12) & 0x3f) | 0x80) as u8);
        parser.buffer.push((((uval >> 6) & 0x3f) | 0x80) as u8);
        parser.buffer.push(((uval & 0x3f) | 0x80) as u8);
        parser.unicode_multi = 0;
        return Ok(());
    }

    if is_low_surrogate(uval) {
        return Err(JError::UNICODE_UNEXPECTED_LOW_SURROGATE);
    }
    if is_high_surrogate(uval) {
        parser.unicode_multi = uval;
        return Ok(());
    }

    if uval < 0x800 {
        parser.buffer.push(((uval >> 6) | 0xc0) as u8);
        parser.buffer.push(((uval & 0x3f) | 0x80) as u8);
    } else {
        parser.buffer.push(((uval >> 12) | 0xe0) as u8);
        parser.buffer.push((((uval >> 6) & 0x3f) | 0x80) as u8);
        parser.buffer.push((((uval >> 0) & 0x3f) | 0x80) as u8);
    }
    Ok(())
}


// ********************************************************************** 
fn act_uc(parser: &mut Parser) -> JResult0 {
	try!(decode_unicode_char(parser));
	parser.state = if parser.unicode_multi > 0 { S::D1 } else { S::_S };
    Ok(())
}

fn act_yb(parser: &mut Parser) -> JResult0 {
	if !parser.config.allow_yaml_comments {
		Err(JError::COMMENT_NOT_ALLOWED)
    } else {
	    parser.save_state = parser.state;
	    Ok(())
    }
}

fn act_cb(parser: &mut Parser) -> JResult0 {
	if !parser.config.allow_c_comments {
		Err(JError::COMMENT_NOT_ALLOWED)
    } else {
    	parser.save_state = parser.state;
	    Ok(())
    }
}

fn act_ce(parser: &mut Parser) -> JResult0 {
	parser.state = if is_state_above_array(parser.save_state) { S::OK } else { parser.save_state };
	Ok(())
}

fn act_ob(parser: &mut Parser, cb: &Callback) -> JResult0 {
	try!(do_callback(parser, cb, Jev::ObjectStart));
	state_push(parser, StackMode::Object);
	parser.expecting_key = true;
    Ok(())
}

fn act_oe(parser: &mut Parser, cb: &Callback) -> JResult0 {
	try!(state_pop(parser, StackMode::Object));
	try!(do_callback(parser, cb, Jev::ObjectEnd));
	parser.expecting_key = false;
	Ok(())
}

fn act_ab(parser: &mut Parser, cb: &Callback) -> JResult0 {
	try!(do_callback(parser, cb, Jev::ArrayStart));
	state_push(parser, StackMode::Array);
	Ok(())
}

fn act_ae(parser: &mut Parser, cb: &Callback) -> JResult0 {
	try!(state_pop(parser, StackMode::Array));
	do_callback(parser, cb, Jev::ArrayEnd)
}

fn act_se(parser: &mut Parser, cb : &Callback) -> JResult0 {
    let ty = if parser.expecting_key { Jev::Key } else { Jev::String };
	try!(do_callback_withbuf(parser, cb, ty));
    parser.buffer.clear();
	parser.state = if parser.expecting_key { S::CO } else { S::OK };
	parser.expecting_key = false;
	Ok(())
}

fn act_sp(parser: &mut Parser) -> JResult0 {
	if parser.stack.len() == 0 {
		Err(JError::COMMA_OUT_OF_STRUCTURE)
    } else {
        parser.state =
            if parser.stack[parser.stack.len() - 1] == StackMode::Object {
                parser.expecting_key = true; S::_K
            } else {
                S::_V
            };
        Ok(())
    }
}
// ********************************************************************** 

fn do_action(parser: &mut Parser, cb: &Callback, next_state: S) -> JResult0 {
    match next_state {
        S::KS => update_simple(parser, None, S::_V),
        S::SP => update_callbk(parser, cb, None, S::__, false, |p| act_sp(p) ),
        S::AB => update_callbk(parser, cb, None, S::_A, false, |p| act_ab(p, cb) ),
        S::AE => update_callbk(parser, cb, None, S::OK, true, |p| act_ae(p, cb) ),
        S::OB => update_callbk(parser, cb, None, S::_O, false, |p| act_ob(p, cb) ),
        S::OE => update_callbk(parser, cb, None, S::OK, true, |p| act_oe(p, cb) ),
        S::CB => update_callbk(parser, cb, None, S::C1, true, |p| act_cb(p) ),
        S::YB => update_callbk(parser, cb, None, S::Y1, true, |p| act_yb(p) ),
        S::CE => update_callbk(parser, cb, None, S::__, false, |p| act_ce(p) ),
        S::FA => update_simple(parser, Some(Jev::False), S::OK),
        S::TR => update_simple(parser, Some(Jev::True),  S::OK),
        S::NU => update_simple(parser, Some(Jev::Null),  S::OK),
        S::DE => update_simple(parser, Some(Jev::Float), S::X1),
        S::DF => update_simple(parser, Some(Jev::Float), S::R1),
        S::SE => update_callbk(parser, cb, None, S::__, false, |p| act_se(p, cb) ),
        S::MX => update_simple(parser, Some(Jev::Int), S::M0),
        S::ZX => update_simple(parser, Some(Jev::Int), S::Z0),
        S::IX => update_simple(parser, Some(Jev::Int), S::I0),
        S::UC => update_callbk(parser, cb, None, S::__, false, |p| act_uc(p) ),
        _     => Ok(())
    }
}

fn get_next_class(parser : &mut Parser, ch : u8) -> JResult<C> {
    if parser.utf8_multibyte_left > 0 {
        if UTF8_CONTINUATION_TABLE[ch as usize] != 0 {
            Err(JError::UTF8)
        } else {
            parser.utf8_multibyte_left = parser.utf8_multibyte_left - 1;
            Ok(C::Other)
        }
    } else {
        let multibytes = UTF8_HEADER_TABLE[ch as usize];
        if multibytes == 0xff {
            Err(JError::UTF8)
        } else {
            let next_class = if multibytes > 0 { C::Other } else { CHARACTER_CLASS[ch as usize] };
            if next_class == C::Error {
                Err(JError::BAD_CHAR)
            } else {
                parser.utf8_multibyte_left = multibytes;
                Ok(next_class)
            }
        }
    }
}

pub fn parse(mut parser: Parser, cb: &Callback) -> Result<(),JError> {
    let ch : u8 = 1;

    let next_class = try!(get_next_class(&mut parser, ch));

    let next_class_num : u8 = unsafe { mem::transmute(next_class) };
    let parser_state_num : u8 = unsafe { mem::transmute(parser.state) };
    let next_state = STATE_TRANS[parser_state_num as usize][next_class_num as usize];

    let buffer_policy = BUFFER_POLICY_TABLE[parser_state_num as usize][next_class_num as usize];
    if next_state == S::__ {
        return Err(JError::UNEXPECTED_CHAR);
    }

    // add char to buffer
    if buffer_policy > 0 {
        if buffer_policy > 2 {
            try!(buffer_push_escape(&mut parser, ch))
        } else {
            try!(buffer_push(&mut parser, ch))
        }
    }

    // move to the next level
    let next_state_num : u8 = unsafe { mem::transmute(next_state) };
    if (next_state_num & 0x80) != 0 {
        try!(do_action(&mut parser, cb, next_state))
    } else {
        parser.state = next_state;
    }
    Ok(())
}

#[test]
fn it_works() {
}
