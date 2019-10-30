#pragma once

#include "IAllocator.h"
#include "Type.h"

class BS_API StackAllocatorBase abstract
{
protected:
	StackAllocatorBase() noexcept;
	StackAllocatorBase(size_t size) noexcept;
	StackAllocatorBase(const StackAllocatorBase& other) noexcept;
	StackAllocatorBase(StackAllocatorBase&& other) noexcept;
	virtual ~StackAllocatorBase() noexcept;

	void* Allocate(size_t size) noexcept;
	void Deallocate(void* ptr, size_t size) noexcept;
	void Clear() noexcept;

	inline size_t GetMaxSize() const noexcept
	{
		return maxNum;
	}

private:
	class MemoryManager* memoryManager;

	uint8* cur;
	uint8* start;

	size_t maxNum;
};

/**
 * @brief Allocator with fixed order of allocation and deallocation
*/
template <class T>
class BS_API StackAllocator final : public StackAllocatorBase, public IAllocator<T>
{
	using Super = StackAllocatorBase;

public:
	inline StackAllocator(const size_t count) noexcept
		: Super(count * sizeof(T)) {}

	inline ~StackAllocator() noexcept = default;

	inline StackAllocator(const StackAllocator& other) noexcept
		: Super(other) {}

	inline StackAllocator(StackAllocator&& other) noexcept
		: Super(std::move(other)) {}

	template <class U>
	StackAllocator(const StackAllocator<U>& other) noexcept
		: Super(other) {}

	T* allocate(size_t n) noexcept override
	{
		return static_cast<T*>(Super::Allocate(n * sizeof(T)));
	}

	void deallocate(T* ptr, size_t n) noexcept override
	{
		Super::Deallocate(static_cast<void*>(ptr), n * sizeof(T));
	}

	void clear() noexcept override
	{
		Super::Clear();
	}

	inline size_t max_size() const noexcept override
	{
		return Super::GetMaxSize() / sizeof(T);
	}
};