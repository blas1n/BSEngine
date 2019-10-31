#pragma once

#include "IAllocatorBase.h"

class BS_API PoolAllocatorBase abstract : public IAllocatorBase
{
protected:
	PoolAllocatorBase(size_t size, bool isSingleFrame = false) noexcept;
	PoolAllocatorBase(const PoolAllocatorBase& other) noexcept;
	PoolAllocatorBase(PoolAllocatorBase&& other) noexcept;
	~PoolAllocatorBase() noexcept;

	void* Allocate(size_t size) noexcept override final;
	void Deallocate(void* ptr, size_t size) noexcept override final;
	void Clear() noexcept override final;

	size_t GetMaxSize() const noexcept override final;
	
	inline bool IsSingleFrame() const noexcept override final
	{
		return isSingleFrameAlloc;
	}

private:
	void Init(size_t size) noexcept;

	class MemoryManager* memoryManager;
	class MarkerMemory* markerMemory;
	bool isSingleFrameAlloc;
};

/**
 * @brief Allocator with fixed allocation size
*/
template <class T>
class BS_API PoolAllocator final : public PoolAllocatorBase {
	using Super = PoolAllocatorBase;

public:
	using value_type = T;

	inline PoolAllocator(const size_t count) noexcept
		: Super(count * sizeof(T)) {}

	inline ~PoolAllocator() noexcept = default;

	inline PoolAllocator(const PoolAllocator& other) noexcept
		: Super(other) {}

	inline PoolAllocator(PoolAllocator&& other) noexcept
		: Super(std::move(other)) {}

	template <class U>
	inline PoolAllocator(const PoolAllocator<U>& other) noexcept
		: Super(other) {}

	inline T* allocate(const size_t n = 1) noexcept
	{
		return static_cast<T*>(Super::Allocate(n * sizeof(T)));
	}

	inline void deallocate(T* ptr, const size_t n = 1) noexcept
	{
		Super::Deallocate(static_cast<void*>(ptr), n * sizeof(T));
	}

	inline void clear() noexcept
	{
		Super::Clear();
	}

	inline size_t max_size() const noexcept
	{
		return Super::GetMaxSize() / sizeof(T);
	}
};