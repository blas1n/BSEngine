#pragma once

#include "IMemory.h"
#include <limits>

/**
 * @brief Memory with fixed order of allocation and deallocation
 * @ref IMemory.h
*/
class BS_API StackMemory final : public IMemory
{
public:
	StackMemory(size_t size) noexcept;
	~StackMemory();

	/**
	 * @param[in] size How much memory to allocate.
	 * @return Address of allocated memory.
	 * @retval nullptr Returned when the memory is full and can no longer be allocated.
	*/
	void* Malloc(size_t size) noexcept override;

	/**
	 * @param[in] ptr New top pointer
	 * @warning If you try to free a pointer that is not at the top, all the pointers above it are freed.
	*/
	void Free(void* ptr) noexcept override;

	void Clear() noexcept override;

	size_t GetAssignedByte() const noexcept override
	{
		return cur - start;
	}

	size_t GetAssignedByte() const noexcept override
	{
		return end - cur;
	}

	size_t GetMaxByte() const noexcept override
	{
		return end - start;
	}

private:
	using byte = unsigned char;

	byte* cur;
	byte* start;
	byte* end;
};