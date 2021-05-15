#pragma once

#include <optional>
#include <vector>
#include "BSBase/Type.h"
#include "Name.h"

using BSBase::uint8;

enum class KeyCode : uint8
{
	Escape = 1,
	One,
	Two,
	Three,
	Four,
	Five,
	Six,
	Seven,
	Eight,
	Nine,
	Zero,
	Minus,
	Equals,
	Back,
	Tab,
	Q,
	W,
	E,
	R,
	T,
	Y,
	U,
	I,
	O,
	P,
	LBracket,
	RBracket,
	Return,
	LControl,
	A,
	S,
	D,
	F,
	G,
	H,
	J,
	K,
	L,
	Semicolon,
	Apostrophe,
	Grave,
	LShift,
	Backslash,
	Z,
	X,
	C,
	V,
	B,
	N,
	M,
	Comma,
	Reriod,
	Slash,
	RShift,
	Multiply,
	LMenu,
	Space,
	Capital,
	F1,
	F2,
	F3,
	F4,
	F5,
	F6,
	F7,
	F8,
	F9,
	F10,
	Numlock,
	Scroll,
	KP7,
	KP8,
	KP9,
	Subtract,
	KP4,
	KP5,
	KP6,
	Add,
	KP1,
	KP2,
	KP3,
	KP0,
	Decimal,
	F11,
	F12,
	F13,
	F14,
	F15,
	Kana,
	Convert,
	NoConvert,
	Yen,
	NumpadEquals,
	CircumFlex,
	At,
	Colon,
	Underline,
	Kanji,
	Stop,
	Ax,
	UnLabeled,
	NumpadeEnter,
	RControl,
	NumpadComma,
	Divide,
	SysRq,
	RMenu,
	Pause,
	Home,
	Up,
	Prior,
	Left,
	Right,
	End,
	Down,
	Next,
	Insert,
	Delete,
	LWin,
	RWin,
	Apps,
	Power,
	Sleep
};

enum class MouseCode : uint8 { L, R, M, X1, X2, X3, X4, X5 };
enum class MouseAxis : uint8 { X, Y, Wheel };

enum class KeyMode : uint8
{
	None	= 0x00,
	Shift	= 0x01,
	Ctrl	= 0x02,
	Alt		= 0x04,
};

constexpr static uint8 ModeNum = 3;

[[nodiscard]] constexpr KeyMode operator&(KeyMode a, KeyMode b) noexcept
{
	return static_cast<KeyMode>(static_cast<int>(a) & static_cast<int>(b));
}

[[nodiscard]] constexpr KeyMode operator|(KeyMode a, KeyMode b) noexcept
{
	return static_cast<KeyMode>(static_cast<int>(a) | static_cast<int>(b));
}

constexpr KeyMode& operator&=(KeyMode& lhs, KeyMode rhs) noexcept
{
	return lhs = lhs & rhs;
}

constexpr KeyMode& operator|=(KeyMode& lhs, KeyMode rhs) noexcept
{
	return lhs = lhs | rhs;
}

[[nodiscard]] NO_ODR Name FromKeyCode(KeyCode code) noexcept
{
	return static_cast<ReservedName>(static_cast<BSBase::int32>
		(ReservedName::Escape) + static_cast<BSBase::int8>(code) - 1);
}

[[nodiscard]] INPUT_API std::vector<Name> FromKeyMode(KeyMode mode) noexcept;
[[nodiscard]] INPUT_API Name FromMouseCode(MouseCode code) noexcept;
[[nodiscard]] INPUT_API Name FromMouseAxis(MouseAxis axis) noexcept;

[[nodiscard]] INPUT_API std::optional<KeyCode> ToKeyCode(Name name) noexcept;
[[nodiscard]] INPUT_API std::optional<KeyMode> ToKeyMode(Name name) noexcept;
[[nodiscard]] INPUT_API std::optional<MouseCode> ToMouseCode(Name name) noexcept;
[[nodiscard]] INPUT_API std::optional<MouseAxis> ToMouseAxis(Name name) noexcept;
