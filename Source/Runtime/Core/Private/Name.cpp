#include "Name.h"
#include <unordered_set>
#include <shared_mutex>
#include <vector>
#include "BSMath/Hash.h"
#include "Assertion.h"

namespace
{
	class NamePool final
	{
	public:
		static NamePool& Get()
		{
			return *inst;
		}

		NamePool()
		{
			reserveMapper.resize(static_cast<BSBase::uint32>(ReservedName::ReserveNum), nullptr);

#define REGISTER_NAME(name, num) reserveMapper[num] = &*nameSet.insert(Impl::ToLower(STR(#name))).first;
#include "ReservedName.inl"
#undef REGISTER_NAME
		}

		const String* Insert(StringView str)
		{
			if (const auto ptr = Find(str))
				return ptr;

			std::unique_lock lock{ mutex };
			return &*nameSet.insert(String(str.data())).first;
		}

		const String* Find(StringView str) const
		{
			std::shared_lock lock{ mutex };

			const auto iter = nameSet.find(String(str.data()));
			return iter != nameSet.end() ? &*iter : nullptr;
		}

		const String* Find(ReservedName name) const
		{
			Assert(name < ReservedName::ReserveNum);
			return reserveMapper[static_cast<BSBase::uint32>(name)];
		}

	private:
		static std::unique_ptr<NamePool> inst;

		std::unordered_set<String, BSMath::Hash<String>> nameSet;
		std::vector<const String*> reserveMapper;
		mutable std::shared_mutex mutex;
	};

	std::unique_ptr<NamePool> NamePool::inst = std::make_unique<NamePool>();
}

namespace Impl
{
	NameBase::NameBase(StringView str)
		: ptr(NamePool::Get().Insert(str)) {}

	NameBase::NameBase(ReservedName name)
		: ptr(NamePool::Get().Find(name)) {}
}
