#include "Name.h"
#include <shared_mutex>

namespace
{
	class NamePool final
	{
	public:
		NamePool();

		Impl::NameEntryId Store(StringView str);

		[[nodiscard]] Impl::NameEntryId Find(StringView str) const;
		[[nodiscard]] Impl::NameEntryId Find(ReservedName name) const;

	private:
		Impl::NameEntryId ReserveCache[static_cast<BSBase::uint32>(ReservedName::ReserveNum)] = {};
		mutable std::shared_mutex mutex;
	};

	NamePool::NamePool()
	{
#define REGISTER_NAME(name, num) ReserveCache[num] = Store(StringView{ STR(#name) });
#include "ReservedName.inl"
#undef REGISTER_NAME
	}

	Impl::NameEntryId NamePool::Store(StringView str)
	{

	}

	Impl::NameEntryId NamePool::Find(StringView str) const
	{

	}

	Impl::NameEntryId NamePool::Find(ReservedName name) const
	{

	}

	alignas(NamePool) static NamePool namePool;
}

namespace Impl
{
	NameEntryId::NameEntryId(ReservedName name)
		: id(namePool.Find(name).id) {}

	bool operator<(NameEntryId lhs, NameEntryId rhs)
	{

	}
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
