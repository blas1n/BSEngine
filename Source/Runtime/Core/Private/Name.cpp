#include "Name.h"
#include <shared_mutex>

namespace
{
	constexpr static BSBase::uint16 MaxNameSize = 1024u;

	struct NameValue final
	{
		explicit NameValue(StringView inName)
			: name(inName), Hash(HashName<Sensitivity>(InName))
		{}

		StringView name;
		FNameHash Hash;
		TOptional<FNameEntryId> ComparisonId;
	};

	class NameEntry final
	{
		Char str[MaxNameSize];
		Impl::NameEntryId id;
		BSBase::uint16 len;
	};

	class NamePool final
	{
	public:
		NamePool();

		Impl::NameEntryId Store(StringView str);

		[[nodiscard]] Impl::NameEntryId Find(StringView str) const;
		[[nodiscard]] Impl::NameEntryId Find(ReservedName name) const;

	private:
		Impl::NameEntryId ReservePool[static_cast<BSBase::uint32>(ReservedName::ReserveNum)] = {};
		mutable std::shared_mutex mutex;
	};

	NamePool::NamePool()
	{
#define REGISTER_NAME(name, num) ReservePool[num] = Store(StringView{ STR(#name) });
#include "ReservedName.inl"
#undef REGISTER_NAME


	}

	Impl::NameEntryId NamePool::Store(StringView str)
	{
		// Note: Don't declare it as a const variable for RVO
		auto existing = Find(str);
		if (existing.id)
			return existing;

		std::unique_lock lock{ mutex };

		bool bAdded = false;
		NameValue nameValue{ str };
		Impl::NameEntryId ComparisonId = shards[ComparisonValue.Hash.ShardIndex].Insert(nameValue, bAdded);

		if (bAdded || EqualsSameDimensions<ENameCase::CaseSensitive>(Resolve(ComparisonId), str))
		{
			DisplayShard.InsertExistingEntry(DisplayValue.Hash, ComparisonId);
			return ComparisonId;
		}
		else
		{
			DisplayValue.ComparisonId = ComparisonId;
			return DisplayShard.Insert(DisplayValue, bAdded);
		}
	}

	Impl::NameEntryId NamePool::Find(StringView str) const
	{
		std::shared_lock lock{ mutex };
	}

	Impl::NameEntryId NamePool::Find(ReservedName name) const
	{
		std::shared_lock lock{ mutex };
	}

	alignas(NamePool) static NamePool namePool;
}

namespace Impl
{
	NameEntryId::NameEntryId(ReservedName name)
		: NameEntryId(namePool.Find(name)) {}
}

Name::Name(StringView str)
	: id(), len(0)
{

}

Name::Name(ReservedName name)
	: id(name), len(0)
{

}

bool Name::IsValid() const
{

}

String Name::ToString() const
{

}
