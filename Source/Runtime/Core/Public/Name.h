#pragma once

#include "BSBase/Type.h"
#include "CharSet.h"

#define REGISTER_NAME(name,num) name = num,
enum class ReservedName : BSBase::uint32
{
#include "ReservedName.inl"
	ReserveNum
};
#undef REGISTER_NAME

namespace Impl
{
	struct NameEntryId final
	{
		constexpr NameEntryId(BSBase::uint32 inId = 0u) : id(inId) {}
		CORE_API NameEntryId(ReservedName name);

		constexpr NameEntryId(const NameEntryId&) = default;
		constexpr NameEntryId(NameEntryId&&) = default;

		constexpr NameEntryId& operator=(const NameEntryId&) = default;
		constexpr NameEntryId& operator=(NameEntryId&&) = default;

		~NameEntryId() = default;

		BSBase::uint32 id;
	};

	[[nodiscard]] constexpr bool operator==(NameEntryId lhs, NameEntryId rhs) { return lhs.id == rhs.id; }
	[[nodiscard]] constexpr bool operator!=(NameEntryId lhs, NameEntryId rhs) { return !(lhs == rhs); }

	[[nodiscard]] CORE_API bool operator<(NameEntryId lhs, NameEntryId rhs);
	[[nodiscard]] NO_ODR bool operator>(NameEntryId lhs, NameEntryId rhs) { return rhs < lhs; }
	[[nodiscard]] NO_ODR bool operator<=(NameEntryId lhs, NameEntryId rhs) { return !(lhs > rhs); }
	[[nodiscard]] NO_ODR bool operator>=(NameEntryId lhs, NameEntryId rhs) { return !(lhs < rhs); }
}

class CORE_API Name final
{
public:
	Name(StringView str);
	Name(ReservedName name = ReservedName::None);

	Name(const Name&) = default;
	Name(Name&&) = default;

	Name& operator=(const Name&) = default;
	Name& operator=(Name&&) = default;

	~Name() = default;

	[[nodiscard]] bool IsValid() const;
	[[nodiscard]] String ToString() const;
	[[nodiscard]] BSBase::uint32 GetLength() const noexcept { return len; }

	friend bool operator==(const Name& lhs, const Name& rhs);
	friend bool operator<(const Name& lhs, const Name& rhs);

private:
	Impl::NameEntryId id;
	BSBase::uint32 len;
};

[[nodiscard]] NO_ODR bool operator==(const Name& lhs, const Name& rhs) { return lhs.id == rhs.id; }
[[nodiscard]] NO_ODR bool operator!=(const Name& lhs, const Name& rhs) { return !(lhs == rhs); }
[[nodiscard]] NO_ODR bool operator<(const Name& lhs, const Name& rhs) { return lhs.id < rhs.id; }
[[nodiscard]] NO_ODR bool operator>(const Name& lhs, const Name& rhs) { return  rhs < lhs; }
[[nodiscard]] NO_ODR bool operator<=(const Name& lhs, const Name& rhs) { return !(lhs > rhs); }
[[nodiscard]] NO_ODR bool operator>=(const Name& lhs, const Name& rhs) { return !(lhs < rhs); }
