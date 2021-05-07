#include "InputCode.h"

Name FromMouseCode(MouseCode code) noexcept
{
	switch (code)
	{
	case MouseCode::L:
		return ReservedName::L;
	case MouseCode::R:
		return ReservedName::R;
	case MouseCode::M:
		return ReservedName::M;
	case MouseCode::X1:
		return ReservedName::X1;
	case MouseCode::X2:
		return ReservedName::X2;
	case MouseCode::X3:
		return ReservedName::X3;
	case MouseCode::X4:
		return ReservedName::X4;
	default:
		return ReservedName::None;
	}
}

Name FromMouseAxis(MouseAxis axis) noexcept
{
	switch (axis)
	{
	case MouseAxis::X:
		return ReservedName::X;
	case MouseAxis::Y:
		return ReservedName::Y;
	default:
		return ReservedName::None;
	}
}

std::optional<KeyCode> ToKeyCode(Name name) noexcept
{
	constexpr static auto Begin = static_cast<BSBase::uint32>(ReservedName::Escape);
	constexpr static auto End = static_cast<BSBase::uint32>(ReservedName::Sleep);
	
	for (BSBase::uint32 i = Begin; i <= End; ++i)
		if (name == static_cast<ReservedName>(i))
			return static_cast<KeyCode>(i - Begin + 1);

	return std::nullopt;
}

std::optional<MouseCode> ToMouseCode(Name name) noexcept
{
	if (name == ReservedName::L)
		return MouseCode::L;
	if (name == ReservedName::R)
		return MouseCode::R;
	if (name == ReservedName::M)
		return MouseCode::M;
	if (name == ReservedName::X1)
		return MouseCode::X1;
	if (name == ReservedName::X2)
		return MouseCode::X2;
	if (name == ReservedName::X3)
		return MouseCode::X3;
	if (name == ReservedName::X4)
		return MouseCode::X4;

	return std::nullopt;
}

std::optional<MouseAxis> ToMouseAxis(Name name) noexcept
{
	if (name == ReservedName::X)
		return MouseAxis::X;
	if (name == ReservedName::Y)
		return MouseAxis::Y;

	return std::nullopt;
}
