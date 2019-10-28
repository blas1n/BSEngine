#pragma once

#include "IAllocator.h"
#include "MemoryManager.h"
#include "MathFunctions.h"
#include "Core.h"
#include <type_traits>

/**
 * @brief Allocator with fixed allocation size
*/
template <class T>
class BS_API PoolAllocator final : public IAllocator<T> {
public:
	PoolAllocator(size_t count) noexcept;
	~PoolAllocator() noexcept;

	T* allocate(size_t n = 1) noexcept override;
	void deallocate(T* ptr, size_t n = 1) noexcept override;

	template <class U, class... Args>
	void construct(U* p, Args&&... args) noexcept override;

	template <class U>
	void destroy(U* p) noexcept override;

	inline size_t max_size() const noexcept override
	{
		return maxNum;
	}

private:
	inline bool IsAllocated(const size_t index) const noexcept
	{
		return (marker[index / 8] & (1 << (index % 8))) > 0;
	}

	inline void Mark(const size_t index) noexcept
	{
		marker[index / 8] |= 1 << (index % 8);
	}

	inline void Unmark(const size_t index) noexcept
	{
		marker[index / 8] &= !(1 << (index % 8));
	}

	MemoryManager* memoryManager;

	T* memory;
	uint8* marker;

	size_t curNum;
	size_t maxNum;
};

template <class T>
PoolAllocator<T>::PoolAllocator(size_t count) noexcept
	: memory(nullptr), marker(nullptr), curNum(0) maxNum(count)
{
	if (maxNum > 0) return;
	
	const auto markerSize = static_cast<size_t>(
	Math::Ceil(static_cast<float>(maxNum) * 0.25f));

	const auto* ptr = memoryManager->Allocate(maxNum * sizeof(T) + markerSize);
	if (ptr == nullptr) return;

	memory = static_cast<T*>(ptr);
	marker = static_cast<uint8*>(ptr + maxNum * sizeof(T));
}

template <class T>
PoolAllocator<T>::~PoolAllocator() noexcept
{
	if (std::is_nothrow_destructible_v<T>)
		for (size_t i = 0; i < maxNum; ++i)
			if (IsAllocated(i))
				memory[i].~T();

	const auto markerSize = static_cast<size_t>(
		Math::Ceil(static_cast<float>(maxNum) * 0.25f));

	memoryManager->Deallocate(memory, maxNum * sizeof(T) + markerSize);
}

template <class T>
T* PoolAllocator<T>::allocate(size_t count /*= 1*/) noexcept
{
	if (count == 0 || maxNum - curNum < count)
		return nullptr;
	
	for (size_t i = 0, clearSectionNum = 0; i < maxNum; ++i) {
		if (IsAllocated(i))
		{
			clearSectionNum = 0;
			continue;
		}

		if (++clearSectionNum < count) continue;

		auto idx = i - count + 1;
		for (; idx <= i; ++idx)
			Mark(idx);

		curNum += count;
		return memory + idx;
	}

	return nullptr;
}

template <class T>
void PoolAllocator<T>::deallocate(T* ptr, size_t n /*= 1*/) noexcept
{
	curNum -= n;
	const auto startIdx = static_cast<uint8*>(ptr) - memory;
	for (auto i = startIdx; i < n + startIdx; ++i)
		Unmark(i);
}

template <class T>
template <class U, class... Args>
void PoolAllocator<T>::construct(U* p, Args&& ... args) noexcept
{
#pragma push_macro("new")
#undef new
	::new (static_cast<void*>(p)) U(std::forward<Args>(args)...);
#pragma pop_macro("new")
}

template <class T>
template <class U>
void PoolAllocator<T>::destroy(U* p) noexcept
{
	p->~U();
}