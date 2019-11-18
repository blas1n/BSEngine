#pragma once

#include "Macro.h"
#include "Type.h"

namespace BE
{
	/// @brief Memory that can be allocated and freed.
	class BS_API HeapMemory final
	{
	public:
		HeapMemory(size_t size) noexcept;
		~HeapMemory() noexcept;

		/**
		 * @brief Allocate memory.
		 * @param n Size to be allocated.
		 * @return Allocated pointer.
		 * @retval nullptr Can not allocate.
		*/
		void* Malloc(size_t n) noexcept;

		/**
		 * @brief Deallocate memory.
		 * @param ptr Pointer to be deallocated.
		 * @param n Size to be deallocated.
		*/
		void Free(void* ptr, size_t n) noexcept;

	private:
		Uint8* memory;
		Uint8* marker;

		size_t curNum;
		size_t maxNum;
		size_t markerSize;
	};
}