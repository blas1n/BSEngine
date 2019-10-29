#pragma once

#include "Macro.h"

/**
 * @brief
 * Allocator interface used by upper layers to access memory.
 * Use this instead of new / delete.
 * Follow C ++ standards for compatibility with STL.
 * @see https://en.cppreference.com/w/cpp/memory/allocator
 * @todo Link memory manager
*/
template <class T>
class BS_API BaseAllocator abstract
{
public:
	using value_type = T;
	using size_type = std::size_t;
	using difference_type = std::ptrdiff_t;
	using propagate_on_container_move_assignment = std::true_type;
	using is_always_equal = std::true_type;
	
	BaseAllocator(size_t count) noexcept;
	virtual ~BaseAllocator() noexcept = default;

	BaseAllocator(const BaseAllocator& other) noexcept;
	BaseAllocator(BaseAllocator&& other) noexcept;

	template <class U>
	BaseAllocator(const BaseAllocator<U>& other) noexcept {}

	virtual T* allocate(size_t) noexcept abstract;
	virtual void deallocate(T*, size_t) noexcept abstract;
	virtual void clear() noexcept abstract;

	template <class U, class... Args>
	void construct(U* p, Args&& ... args) noexcept;

	template <class U>
	void destroy(U* p) noexcept;

	inline size_t max_size() const noexcept { return maxNum; }

protected:
	inline class MemoryManager* GetMemoryManager() const noexcept { return memoryManager; }

private:
	MemoryManager* memoryManager;
	size_t maxNum;
};

template <class T>
BaseAllocator<T>::BaseAllocator(size_t size) noexcept
	: memoryManager(nullptr), maxNum(size) {}

template <class T>
BaseAllocator<T>::BaseAllocator(const BaseAllocator& other) noexcept
	: memoryManager(other.memoryManager),
	maxNum(other.maxNum) {}

template <class T>
BaseAllocator<T>::BaseAllocator(BaseAllocator&& other) noexcept
	: memoryManager(std::move(other.memoryManager)),
	maxNum(std::move(other.maxNum)) {}

template <class T>
template <class U, class... Args>
void BaseAllocator<T>::construct(U* p, Args&& ... args) noexcept
{
#pragma push_macro("new")
#undef new
	if (std::is_nothrow_constructible_v<U>)
		::new (static_cast<void*>(p)) U(std::forward<Args>(args)...);
#pragma pop_macro("new")
}

template <class T>
template <class U>
void BaseAllocator<T>::destroy(U* p) noexcept
{
	if (std::is_nothrow_destructible_v<U>)
		p->~U();
}