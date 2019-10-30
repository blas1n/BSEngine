#pragma once

#include "IAllocatorBase.h"
#include "Type.h"

class BS_API StackAllocatorBase abstract : public IAllocatorBase
{
protected:
	StackAllocatorBase() noexcept;
	StackAllocatorBase(size_t size) noexcept;
	StackAllocatorBase(const StackAllocatorBase& other) noexcept;
	StackAllocatorBase(StackAllocatorBase&& other) noexcept;
	~StackAllocatorBase() noexcept;

	void* Allocate(size_t size) noexcept override final;
	void Deallocate(void* ptr, size_t size) noexcept override final;
	void Clear() noexcept override final;

	inline size_t GetMaxSize() const noexcept override final
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
class BS_API StackAllocator final : public StackAllocatorBase
{
	using Super = StackAllocatorBase;

public:
	using value_type = T;

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

	T* allocate(size_t n) noexcept
	{
		return static_cast<T*>(Super::Allocate(n * sizeof(T)));
	}

	void deallocate(T* ptr, size_t n) noexcept
	{
		Super::Deallocate(static_cast<void*>(ptr), n * sizeof(T));
	}

	void clear() noexcept
	{
		Super::Clear();
	}

	inline size_t max_size() const noexcept
	{
		return Super::GetMaxSize() / sizeof(T);
	}
};