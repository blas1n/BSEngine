#pragma once

#include "Macro.h"
#include "Type.h"
#include <vector>

namespace BE
{
	template <class ContainerType>
	class BS_API RandomAccessIterator final
	{
	public:
		RandomAccessIterator(ContainerType& inContainer, size_t inIdx = 0)
			: container{ inContainer }, idx{ inIdx } {}

		RandomAccessIterator(RandomAccessIterator& other) = default;
		RandomAccessIterator(RandomAccessIterator&& other) = default;

		RandomAccessIterator& operator=(RandomAccessIterator& other) = default;
		RandomAccessIterator& operator=(RandomAccessIterator&& other) = default;

		inline RandomAccessIterator& operator++() noexcept
		{
			++idx;
			return *this;
		}

		inline RandomAccessIterator operator++(int) noexcept
		{
			auto tmp{ *this };
			++idx;
			return tmp;
		}

		inline RandomAccessIterator& operator--() noexcept
		{
			--idx;
			return *this;
		}

		inline RandomAccessIterator operator--(int) noexcept
		{
			auto tmp{ *this };
			--idx;
			return tmp;
		}

		inline RandomAccessIterator& operator+=(Int32 offset) noexcept
		{
			idx += offset;
			return *this;
		}

		inline RandomAccessIterator& operator-=(Int32 offset) noexcept
		{
			idx -= offset;
			return *this;
		}

		inline auto& operator* () const noexcept
		{
			return container[idx];
		}

		inline auto* operator->() const noexcept
		{
			return &container[idx];
		}

		inline operator bool() const noexcept
		{
			//return container.IsValidIndex(idx);
		}

		inline void Reset() noexcept
		{
			idx = 0;
		}

		inline void SetToEnd() noexcept
		{
			idx = container.Size();
		}

		inline friend bool operator==(const RandomAccessIterator& lhs, const RandomAccessIterator& rhs)
		{
			return &lhs.container == &rhs.container && lhs.idx == rhs.idx;
		}

		inline friend bool operator!=(const RandomAccessIterator& lhs, const RandomAccessIterator& rhs)
		{
			return !(lhs == rhs);
		}

	private:
		ContainerType& container;
		size_t idx;
	};

	template <class ContainerType>
	RandomAccessIterator<ContainerType> operator+(
		RandomAccessIterator<ContainerType> iter, Int32 offset) noexcept
	{
		return iter += offset;
	}

	template <class ContainerType>
	RandomAccessIterator<ContainerType> operator-(
		RandomAccessIterator<ContainerType> iter, Int32 offset) noexcept
	{
		return iter -= offset;
	}
}