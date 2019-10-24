#pragma once

#include "IMemory.h"
#include <memory>

/// @todo Replace to own math library.
#include <cmath>

/**
 * @brief Memory with fixed allocation size
*/
template <size_t Size>
class BS_API PoolMemory : public IMemory {
public:
	PoolMemory(size_t count) noexcept;
	~PoolMemory();

	void* Malloc(size_t count = 1) noexcept override;
	void Free(void* ptr) noexcept override;
	void Clear() noexcept override;

	size_t GetAssignedByte() const noexcept override;
	size_t GetAssignableByte() const noexcept override;
	size_t GetMaxByte() const noexcept override;

private:
	using byte = unsigned char;

	/**
	 * @brief Initialize space to be allocated and update data
	 * @detail Start and end is index (half-closed interval)
	*/
	void Allocation(size_t start, size_t end) noexcept;

	byte* memory;

	size_t* marker;
	size_t index;

	size_t curNum;
	size_t maxNum;
};

template <size_t Size>
PoolMemory<Size>::PoolMemory(size_t count) noexcept
	: memory(nullptr), marker(nullptr), index(0), curNum(0), maxNum(count)
{
	if (maxNum > 0)
	{
		memory = static_cast<byte*>(std::malloc(maxNum * Size));
		marker = static_cast<size_t*>(std::malloc(maxNum * sizeof(size_t)));

		if (marker != nullptr)
			std::memset(marker, 0, maxNum * sizeof(size_t));
	}
}

template <size_t Size>
PoolMemory<Size>::~PoolMemory()
{
	std::free(marker);
	std::free(memory);
}

template <size_t Size>
void* PoolMemory<Size>::Malloc(size_t count /*= 1*/) noexcept
{
	if (count == 0 || maxNum - curNum < count)
		return nullptr;
	
	for (size_t i = 0, clearSectionNum = 0; i < maxNum; ++i) {
		if (marker[i] == 0)
		{
			if (++clearSectionNum >= count)
			{
				const auto startIndex = i - count + 1;
				Allocation(startIndex, i + 1);
				return memory + (startIndex * Size);
			}
		}
		else clearSectionNum = 0;
	}

	/// @todo Implement defragmentation.
	
	return nullptr;
}

template <size_t Size>
void PoolMemory<Size>::Free(void* ptr) noexcept
{
	const auto diff = static_cast<byte*>(ptr) - memory;
	check(diff >= 0 && static_cast<size_t>(diff) < maxNum * Size && (diff % Size) == 0);

	const auto startIndex = diff / Size;
	const auto mark = marker[startIndex];

	size_t i;
	for (i = startIndex; i < maxNum && marker[i] == mark; ++i)
		marker[i] = 0;

	curNum -= i - startIndex;
	--index;
}

template <size_t Size>
void PoolMemory<Size>::Clear() noexcept
{
	const auto n = maxNum * Size;
	std::memset(memory, 0, n);
	std::memset(marker, 0, n);
	index = curNum = 0;
}

template <size_t Size>
size_t PoolMemory<Size>::GetAssignedByte() const noexcept
{
	return curNum * Size;
}

template <size_t Size>
size_t PoolMemory<Size>::GetAssignableByte() const noexcept
{
	return (maxNum - curNum) * Size;
}

template <size_t Size>
size_t PoolMemory<Size>::GetMaxByte() const noexcept
{
	return maxNum * Size;
}

template <size_t Size>
void PoolMemory<Size>::Allocation(size_t start, size_t end) noexcept
{
	++index;

	const auto diff = end - start;
	curNum += diff;

	std::memset(memory + (start * Size), 0, diff * Size);
	for (size_t i = start; i < end; ++i)
		marker[i] = index;
}