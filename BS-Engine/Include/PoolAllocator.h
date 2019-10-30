#pragma once

#include "IAllocator.h"

class BS_API PoolAllocatorBase abstract
{
protected:
	PoolAllocatorBase() noexcept = default;
	PoolAllocatorBase(size_t size) noexcept;
	PoolAllocatorBase(const PoolAllocatorBase& other) noexcept;
	PoolAllocatorBase(PoolAllocatorBase&& other) noexcept;
	~PoolAllocatorBase() noexcept;

	void* Allocate(size_t size) noexcept;
	void Deallocate(void* ptr, size_t size) noexcept;
	void Clear() noexcept;

	size_t GetMaxSize() const noexcept;

private:
	void Init(size_t size) noexcept;

	class MemoryManager* memoryManager;
	class MarkerMemory* markerMemory;
};

/**
 * @brief Allocator with fixed allocation size
*/
template <class T>
class BS_API PoolAllocator final : protected PoolAllocatorBase, public IAllocator<T> {
	using Super = PoolAllocatorBase;

public:
	template <class U>
	struct rebind { using other = PoolAllocator<U>; };

	inline PoolAllocator(const size_t count) noexcept
		: Super(count * sizeof(T)) {}

	inline ~PoolAllocator() noexcept {}

	inline PoolAllocator(const PoolAllocator& other) noexcept
		: Super(other) {}

	inline PoolAllocator(PoolAllocator&& other) noexcept
		: Super(std::move(other)) {}

	template <class U>
	inline PoolAllocator(const PoolAllocator<U>& other) noexcept
		: Super() {}

	inline T* allocate(const size_t n = 1) noexcept override
	{
		return static_cast<T*>(Super::Allocate(n * sizeof(T)));
	}

	inline void deallocate(T* ptr, const size_t n = 1) noexcept override
	{
		Super::Deallocate(static_cast<void*>(ptr), n * sizeof(T));
	}

	inline void clear() noexcept override
	{
		Super::Clear();
	}

	inline size_t max_size() const noexcept override
	{
		return Super::GetMaxSize() / sizeof(T);
	}
};