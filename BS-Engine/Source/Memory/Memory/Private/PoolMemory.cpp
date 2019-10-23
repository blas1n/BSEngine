#include "../Public/PoolMemory.h"
#include <memory>
#include <cmath>

template <size_t Size>
PoolMemory<Size>::PoolMemory(size_t count) noexcept
	: memory(nullptr), marker(nullptr), curNum(0), maxNum(count)
{
	if (maxNum > 0)
	{
		memory = static_cast<byte*>(std::malloc(Size * maxNum));
		marker = new size_t[maxNum];
	}
}

template <size_t Size>
PoolMemory<Size>::~PoolMemory()
{
	delete[] marker;
	std::free(memory);
}

template <size_t Size>
void* PoolMemory<Size>::Malloc(size_t count /*= 1*/)
{
	if (maxNum - curNum < count)
		return nullptr;

	size_t clearSectionNum = 0;
	for (size_t i = 0; i < maxNum; ++i) {
		if (marker[i] == 0)
		{
			if (++clearSectionNum >= count)
			{
				const auto ret = memory + i - count + 1;
				std::memset(ret, 0, count);
				return ret;
			}
		}
		else clearSectionNum = 0;
	}

	/// @todo Implement defragmentation.
	curNum++;
	return nullptr;
}

template <size_t Size>
void PoolMemory<Size>::Free(void* ptr)
{
	const auto diff = static_cast<byte*>(ptr) - memory;
	check(diff < 0 && static_cast<size_t>(diff) >= maxNum && (diff % Size) == 0);

	const auto index = diff / Size;
	const auto mark = marker[index];

	for (size_t i = index; i < maxNum; ++i) {
		if (marker[i] != mark)
			break;

		marker[i] = 0;
	}

	curNum--;
}

template <size_t Size>
void PoolMemory<Size>::Clear()
{
	std::memset(memory, 0, maxNum);
	std::memset(marker, 0, maxNum);
	curNum = 0;
}

namespace
{
	void FixedLink()
	{
		PoolMemory<1> pm{ 1 };
	}
}