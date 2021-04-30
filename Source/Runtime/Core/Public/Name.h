#pragma once

#include <algorithm>
#include <locale>
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
		[[nodiscard]] size_t GetLength() const noexcept { return ptr->size(); }

		friend bool operator==(const NameBase& lhs, const NameBase& rhs) noexcept;
		friend bool operator<(const NameBase& lhs, const NameBase& rhs);

	private:
		const ::String* ptr;
	};

	NO_ODR ::String ToLower(StringView str)
	{
		::String ret(str.data());
		std::transform(ret.begin(), ret.end(), ret.begin(), [](Char c) { return std::tolower(c, std::locale{}); });
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

template <NameCase Sensitivity>
class CasedName;

template <>
class CORE_API CasedName<NameCase::IgnoreCase> final : public Impl::NameBase
{
public:
	CasedName(StringView str)
		: NameBase(Impl::ToLower(str)) {}

	CasedName(ReservedName name = ReservedName::None)
		: NameBase(name) {}
};

template <>
class CORE_API CasedName<NameCase::CompareCase> final : public Impl::NameBase
{
public:
	CasedName(StringView str)
		: NameBase(str) {}

	CasedName(ReservedName name = ReservedName::None)
		: NameBase(name) {}
};

using Name = CasedName<NameCase::IgnoreCase>;
