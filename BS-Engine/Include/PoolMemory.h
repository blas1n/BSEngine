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

	void Allocation(byte* start, byte* end);

	byte* memory;

	size_t* marker;
	size_t index;

	size_t curNum;
	size_t maxNum;
};

template <size_t Size>
PoolMemory<Size>::PoolMemory(size_t count) noexcept
	: memory(nullptr), marker(nullptr), index(0), curNum(0), maxNum(count * Size)
{
	if (maxNum > 0)
	{
		memory = static_cast<byte*>(std::malloc(maxNum));
		marker = static_cast<size_t*>(std::malloc(count * sizeof(size_t)));

		if (marker != nullptr)
			std::memset(marker, 0, count * sizeof(size_t));
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
	if (count == 0 || GetAssignableByte() < count * Size)
		return nullptr;
	
	for (size_t i = 0, clearSectionNum = 0; i < maxNum; ++i) {
		if (marker[i] == 0)
		{
			if (++clearSectionNum >= count)
			{
				const auto startIndex = i - count + 1;
				const auto ret = memory + startIndex;
				Allocation(ret, memory + i);
				return ret;
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
	check(diff >= 0 && static_cast<size_t>(diff) < maxNum && (diff % Size) == 0);

	const auto startIndex = diff / Size;
	const auto mark = marker[startIndex];

	size_t i;
	for (i = startIndex; i < maxNum && marker[i] == mark; ++i)
		marker[i] = 0;

	curNum -= i * Size;
	--index;
}

template <size_t Size>
void PoolMemory<Size>::Clear() noexcept
{
	std::memset(memory, 0, maxNum);
	std::memset(marker, 0, maxNum);
	index = curNum = 0;
}

template <size_t Size>
size_t PoolMemory<Size>::GetAssignedByte() const noexcept
{
	return curNum;
}

template <size_t Size>
size_t PoolMemory<Size>::GetAssignableByte() const noexcept
{
	return maxNum - curNum;
}

template <size_t Size>
size_t PoolMemory<Size>::GetMaxByte() const noexcept
{
	return maxNum;
}

template <size_t Size>
void PoolMemory<Size>::Allocation(byte* start, byte* end)
{
	++index;

	const auto diff = end - start + 1;
	curNum += diff * Size;

	std::memset(start, 0, end - start + 1);
	for (size_t i = start - memory; i <= end - memory; ++i)
		marker[i] = index;
}