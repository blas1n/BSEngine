#pragma once

#include "MemoryManager.h"
#include "MemoryManagerAccesser.h"

namespace BE
{
	template <class T>
	class BS_API HeapAllocator : public MemoryManagerAccesser
	{
	public:
		HeapAllocator() noexcept = default;
		HeapAllocator(const HeapAllocator& other) noexcept = default;
		~HeapAllocator() noexcept = default;

		template <class U>
		HeapAllocator(const HeapAllocator<U>& other) noexcept {}

		inline T* Allocate(const size_t n = 1) noexcept
		{
			return static_cast<T*>(MemoryManagerAccesser::Get()
				->GetHeapMemory().Allocate(n));
		}

		inline void Deallocate(T* const ptr, const size_t n = 1) noexcept
		{
			MemoryManagerAccesser::Get()->GetHeapMemory().
				Deallocate(static_cast<void*>(ptr), n);
		}
	};
}