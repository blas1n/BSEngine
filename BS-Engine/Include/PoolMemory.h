#pragma once

#include "IMemory.h"
#include <vector>

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

	size_t GetAssignedByte() const noexcept override
	{
		return curNum * Size;
	}

	size_t GetAssignedByte() const noexcept override
	{
		return (maxNum - curNum) * Size;
	}

	size_t GetMaxByte() const noexcept override
	{
		return maxNum * Size;
	}

private:
	using byte = unsigned char;

	byte* memory;
	size_t* marker;

	size_t curNum;
	size_t maxNum;
};