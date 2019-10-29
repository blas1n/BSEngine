#pragma once

#include "Core.h"
#include "BaseAllocator.h"
#include "MathFunctions.h"
#include <type_traits>

/**
 * @brief Allocator with fixed allocation size
*/
template <class T>
class BS_API PoolAllocator final : public BaseAllocator<T> {
public:
	PoolAllocator(size_t count) noexcept;
	~PoolAllocator() noexcept;

	T* allocate(size_t n = 1) noexcept override;
	void deallocate(T* ptr, size_t n = 1) noexcept override;
	void clear() noexcept override;

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

	T* memory;
	uint8* marker;

	size_t curNum;
};

template <class T>
PoolAllocator<T>::PoolAllocator(size_t count) noexcept
	: BaseAllocator<T>(count),
	memory(nullptr),
	marker(nullptr),
	curNum(0)
{
	if (count == 0) return;
	
	const auto markerSize = static_cast<size_t>(
	Math::Ceil(static_cast<float>(count) * 0.25f));

	const auto* ptr = GetMemoryManager()->Allocate(count * sizeof(T) + markerSize);
	if (ptr == nullptr) return;

	memory = static_cast<T*>(ptr);
	marker = static_cast<uint8*>(ptr + count * sizeof(T));
}

template <class T>
PoolAllocator<T>::~PoolAllocator() noexcept
{
	const auto n = max_size();

	if (std::is_nothrow_destructible_v<T>)
		for (size_t i = 0; i < n; ++i)
			if (IsAllocated(i))
				memory[i].~T();

	const auto markerSize = static_cast<size_t>(
		Math::Ceil(static_cast<float>(n) * 0.25f));

	GetMemoryManager()->Deallocate(memory, n * sizeof(T) + markerSize);
}

template <class T>
T* PoolAllocator<T>::allocate(size_t count /*= 1*/) noexcept
{
	const auto n = max_size();

	if (count == 0 || n - curNum < count)
		return nullptr;
	
	for (size_t i = 0, clearSectionNum = 0; i < n; ++i) {
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
void PoolAllocator<T>::clear() noexcept
{
	if (std::is_nothrow_destructible_v<T>)
		for (size_t i = 0; i < n; ++i)
			if (IsAllocated(i))
				memory[i].~T();
}