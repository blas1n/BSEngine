#pragma once

#include <algorithm>
#include "BSBase/Type.h"
#include "CharSet.h"

enum class NameCase : BSBase::uint8
{
	IgnoreCase, CompareCase
};

#define REGISTER_NAME(name,num) name = num,
enum class ReservedName : BSBase::uint32
{
#include "ReservedName.inl"
	ReserveNum
};
#undef REGISTER_NAME

namespace Impl
{
	class CORE_API NameBase
	{
	public:
		NameBase(StringView str);
		NameBase(ReservedName name);

		NameBase(const NameBase&) = default;
		NameBase(NameBase&&) noexcept = default;

		NameBase& operator=(const NameBase&) = default;
		NameBase& operator=(NameBase&&) noexcept = default;

		~NameBase() = default;

		[[nodiscard]] const ::String& ToString() const { return *ptr; }
		[[nodiscard]] BSBase::uint32 GetLength() const noexcept { return ptr->size(); }

		friend bool operator==(const NameBase& lhs, const NameBase& rhs) noexcept;
		friend bool operator<(const NameBase& lhs, const NameBase& rhs);

	private:
		const ::String* ptr;
	};

	NO_ODR ::String ToLower(StringView str)
	{
		::String ret(str.data());
		std::transform(ret.begin(), ret.end(), ret.begin(), [](char c) { return std::tolower(c); });
		return ret;
	}

	[[nodiscard]] NO_ODR bool operator==(const Impl::NameBase& lhs, const Impl::NameBase& rhs) noexcept { return lhs.ptr == rhs.ptr; }
	[[nodiscard]] NO_ODR bool operator!=(const Impl::NameBase& lhs, const Impl::NameBase& rhs) noexcept { return !(lhs == rhs); }

	/// @warning That function has the same cost as string.
	[[nodiscard]] NO_ODR bool operator<(const Impl::NameBase& lhs, const Impl::NameBase& rhs) { return lhs.ToString() < rhs.ToString(); }

	/// @warning That function has the same cost as string.
	[[nodiscard]] NO_ODR bool operator>(const Impl::NameBase& lhs, const Impl::NameBase& rhs) { return  rhs < lhs; }

	/// @warning That function has the same cost as string.
	[[nodiscard]] NO_ODR bool operator<=(const Impl::NameBase& lhs, const Impl::NameBase& rhs) { return !(lhs > rhs); }

	/// @warning That function has the same cost as string.
	[[nodiscard]] NO_ODR bool operator>=(const Impl::NameBase& lhs, const Impl::NameBase& rhs) { return !(lhs < rhs); }
}

template <NameCase Sensitivity = NameCase::IgnoreCase>
class Name final : public Impl::NameBase
{
public:
	Name(StringView str)
		: NameBase(Impl::ToLower(str)) {}

	Name(ReservedName name = ReservedName::None)
		: NameBase(name) {}
};

template <>
class Name<NameCase::CompareCase> final : public Impl::NameBase
{
public:
	Name(StringView str)
		: NameBase(str) {}

	Name(ReservedName name = ReservedName::None)
		: NameBase(name) {}
};
