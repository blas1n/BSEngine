#pragma once

#include "MemoryManager.h"

/*
 * @brief Temp memory manager getter
 * @todo Change memory manager linker with system layer
*/
static MemoryManager* GetMemoryManager() noexcept
{
	static MemoryManager memoryManager;
	static bool isInit = false;
	if (!isInit)
	{
		memoryManager.Init();
		isInit = true;
	}
	return &memoryManager;
}

/**
 * @brief Allocator with fixed allocation size
*/
template <class T>
class BS_API Allocator final {
public:
	using value_type = T;
	using size_type = std::size_t;
	using difference_type = std::ptrdiff_t;
	using propagate_on_container_move_assignment = std::true_type;
	using is_always_equal = std::true_type;

	inline Allocator() noexcept = default;
	inline Allocator(const Allocator & other) noexcept = default;
	inline ~Allocator() noexcept = default;

	template <class U>
	inline Allocator(const Allocator<U>& other) noexcept {}

	inline T* allocate(const size_t n = 1) noexcept
	{
		return static_cast<T*>(GetMemoryManager()->Allocate(n * sizeof(T)));
	}

	inline void deallocate(T* ptr, const size_t n = 1) noexcept
	{
		return GetMemoryManager()->Deallocate(static_cast<void*>(ptr), n * sizeof(T));
	}
};