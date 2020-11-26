#pragma once

#include <algorithm>
#include <string>
#include <vector>

namespace ArenaBoss::IteratorFinder
{
	namespace Impl
	{
		template <class T, class U, bool IsFindSame>
		decltype(auto) FindIter(std::vector<T*>& objects, const U& value)
		{
			if (objects.empty())
				throw std::exception{ "No object of that name exists" };
			
			const auto iter = std::lower_bound(objects.begin(), objects.end(), value,
				[](const auto& lhs, const auto& rhs) { return *lhs < rhs; });
			
			if (iter == objects.end())
				throw std::exception{ "No object of that name exists" };

			if constexpr (IsFindSame)
			{
				if (value != **iter)
					throw std::exception{ "No object of that name exists" };
			}
			else
			{
				if (value == **iter)
					throw std::exception{ "Object with the same name already exists" };
			}

			return iter;
		}
	}

	template <class T, class U>
	decltype(auto) FindSameIterator(std::vector<T*>& objects, const U& value)
	{
		return Impl::FindIter<T, U, true>(objects, value);
	}

	template <class T, class U>
	decltype(auto) FindLowerIterator(std::vector<T*>& objects, const U& value)
	{
		return Impl::FindIter<T, U, false>(objects, value);
	}
}