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
		NameEntryId(BSBase::uint32 inId = 0u) : id(inId) {}
		CORE_API NameEntryId(ReservedName name);

		NameEntryId(const NameEntryId&) = default;
		NameEntryId(NameEntryId&&) noexcept = default;

		NameEntryId& operator=(const NameEntryId&) = default;
		NameEntryId& operator=(NameEntryId&&) noexcept = default;

		~NameEntryId() = default;

		BSBase::uint32 id;
	};
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

[[nodiscard]] NO_ODR bool operator==(const Name& lhs, const Name& rhs) noexcept { return lhs.id.id == rhs.id.id; }
[[nodiscard]] NO_ODR bool operator!=(const Name& lhs, const Name& rhs) noexcept { return !(lhs == rhs); }

/// @warning That function has the same cost as string.
[[nodiscard]] NO_ODR bool operator<(const Name& lhs, const Name& rhs) { return lhs.ToString() < rhs.ToString(); }

/// @warning That function has the same cost as string.
[[nodiscard]] NO_ODR bool operator>(const Name& lhs, const Name& rhs) { return  rhs < lhs; }

/// @warning That function has the same cost as string.
[[nodiscard]] NO_ODR bool operator<=(const Name& lhs, const Name& rhs) { return !(lhs > rhs); }

/// @warning That function has the same cost as string.
[[nodiscard]] NO_ODR bool operator>=(const Name& lhs, const Name& rhs) { return !(lhs < rhs); }
