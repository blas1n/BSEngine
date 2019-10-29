#pragma once

#include "Macro.h"
#include "MemoryManager.h"

/**
 * @brief
 * Allocator interface used by upper layers to access memory.
 * Use this instead of new / delete.
 * Follow C ++ standards for compatibility with STL.
 * @see https://en.cppreference.com/w/cpp/memory/allocator
*/
template <class T>
class BS_API BaseAllocator abstract
{
public:
	BaseAllocator(size_t count) noexcept;
	virtual ~BaseAllocator() noexcept = default;

	virtual T* allocate(size_t) noexcept = 0;
	virtual void deallocate(T*, size_t) noexcept = 0;
	virtual void clear() noexcept = 0;

	template <class U, class... Args>
	void construct(U* p, Args&& ... args) noexcept;

	template <class U>
	void destroy(U* p) noexcept;

	size_t max_size() const noexcept;

protected:
	MemoryManager* GetMemoryManager() const noexcept;

private:
	MemoryManager* memoryManager;
	size_t maxNum;
};

template <class T>
BaseAllocator<T>::BaseAllocator(size_t size) noexcept
	: maxNum(size) {

	memoryManager = new MemoryManager();
}

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

template <class T>
inline size_t BaseAllocator<T>::max_size() const noexcept
{
	return maxNum;
}

template <class T>
inline MemoryManager* BaseAllocator<T>::GetMemoryManager() const noexcept
{
	return memoryManager;
}