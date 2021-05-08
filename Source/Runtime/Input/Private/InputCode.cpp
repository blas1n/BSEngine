#include "InputCode.h"
#include <unordered_map>

std::vector<Name> FromKeyMode(KeyMode mode) noexcept
{
	std::vector<Name> ret;

	for (uint8 i = 0; i < 8; ++i)
		if ((mode & static_cast<KeyMode>(1 << i)) != KeyMode::None)
			ret.push_back(static_cast<ReservedName>(static_cast<BSBase::uint32>(ReservedName::Shift) + i));

	return ret;
}

Name FromMouseCode(MouseCode code) noexcept
{
	constexpr static ReservedName names[]
	{
		ReservedName::L, ReservedName::R, ReservedName::M, ReservedName::X1,
		ReservedName::X2, ReservedName::X3, ReservedName::X4, ReservedName::X5
	};

	if (static_cast<uint8>(code) > static_cast<uint8>(MouseCode::X5))
		return ReservedName::None;
	
	return names[static_cast<uint8>(code)];
}

Name FromMouseAxis(MouseAxis axis) noexcept
{
	constexpr static ReservedName names[]
	{
		ReservedName::X, ReservedName::Y, ReservedName::Wheel
	};

	if (static_cast<uint8>(axis) > static_cast<uint8>(MouseAxis::Wheel))
		return ReservedName::None;

	return names[static_cast<uint8>(axis)];
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

std::optional<KeyMode> ToKeyMode(Name name) noexcept
{
	static std::unordered_map<Name, KeyMode, BSMath::Hash<Name>> modes
	{
		std::make_pair(Name{ ReservedName::None }, KeyMode::None),
		std::make_pair(Name{ ReservedName::Shift }, KeyMode::Shift),
		std::make_pair(Name{ ReservedName::Ctrl }, KeyMode::Ctrl),
		std::make_pair(Name{ ReservedName::Alt }, KeyMode::Alt),
		std::make_pair(Name{ ReservedName::Gui }, KeyMode::Gui),
		std::make_pair(Name{ ReservedName::Num }, KeyMode::Num),
		std::make_pair(Name{ ReservedName::Caps }, KeyMode::Caps),
		std::make_pair(Name{ ReservedName::Mode }, KeyMode::Mode)
	};
	
	const auto iter = modes.find(name);
	if (iter != modes.cend())
		return iter->second;

	return std::nullopt;
}

std::optional<MouseCode> ToMouseCode(Name name) noexcept
{
	static std::unordered_map<Name, MouseCode, BSMath::Hash<Name>> codes
	{
		std::make_pair(Name{ ReservedName::L }, MouseCode::L),
		std::make_pair(Name{ ReservedName::R }, MouseCode::R),
		std::make_pair(Name{ ReservedName::M }, MouseCode::M),
		std::make_pair(Name{ ReservedName::X1 }, MouseCode::X1),
		std::make_pair(Name{ ReservedName::X2 }, MouseCode::X2),
		std::make_pair(Name{ ReservedName::X3 }, MouseCode::X3),
		std::make_pair(Name{ ReservedName::X4 }, MouseCode::X4),
		std::make_pair(Name{ ReservedName::X5 }, MouseCode::X5)
	};

	const auto iter = codes.find(name);
	if (iter != codes.cend())
		return iter->second;

	return std::nullopt;
}

std::optional<MouseAxis> ToMouseAxis(Name name) noexcept
{
	if (name == ReservedName::X)
		return MouseAxis::X;
	if (name == ReservedName::Y)
		return MouseAxis::Y;
	if (name == ReservedName::Wheel)
		return MouseAxis::Wheel;

	return std::nullopt;
}
