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

	PoolAllocator(const PoolAllocator& other) noexcept;
	PoolAllocator(PoolAllocator&& other) noexcept;

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
	size_t markerSize;
};

template <class T>
PoolAllocator<T>::PoolAllocator(size_t count) noexcept
	: BaseAllocator<T>(count),
	memory(nullptr),
	marker(nullptr),
	curNum(0),
	markerSize(Math::Ceil(static_cast<float>(count) * 0.25f))
{
	if (count == 0) return;
	
	const auto* ptr = GetMemoryManager()->Allocate(count * sizeof(T) + markerSize);
	if (ptr == nullptr) return;

	memory = static_cast<T*>(ptr);
	marker = static_cast<uint8*>(ptr + count * sizeof(T));
}

template <class T>
PoolAllocator<T>::~PoolAllocator() noexcept
{
	GetMemoryManager()->Deallocate(memory, max_size() * sizeof(T) + markerSize);
}

template <class T>
PoolAllocator<T>::PoolAllocator(const PoolAllocator<T>& other) noexcept
	: PoolAllocator(other.max_size()) {}

template <class T>
PoolAllocator<T>::PoolAllocator(PoolAllocator<T>&& other) noexcept
	: PoolAllocator<T>(std::move(other)),
	memory(std::move(other.memory)),
	marker(std::move(other.marker)),
	curNum(std::move(other.curNum)),
	markerSize(std::move(other.markerSize)) {}

template <class T>
T* PoolAllocator<T>::allocate(size_t n /*= 1*/) noexcept
{
	const auto max = max_size();

	if (n == 0 || max - curNum < n)
		return nullptr;
	
	for (size_t i = 0, clearSectionNum = 0; i < max; ++i) {
		if (IsAllocated(i))
		{
			clearSectionNum = 0;
			continue;
		}

		if (++clearSectionNum < n) continue;

		auto idx = i - n + 1;
		for (; idx <= i; ++idx)
			Mark(idx);

		curNum += n;
		std::memset(memory + idx, 0, n * sizeof(T));
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
	std::memset(marker, 0, markerSize);
}