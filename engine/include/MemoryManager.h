#pragma once

#include "Macro.h"
#include "Type.h"

namespace BE
{
	/**
	 * @brief Manager that manages all the memory used by the game engine.
	 * @todo Build garbage collector. And convert memory sizing to data driven.
	*/
	class BS_API MemoryManager final
	{
	public:
		/**
		 * @brief Maximum amount of memory the game engine will use.
		 * @warning The game terminates when the amount of memory allocation becomes larger.
		*/
		constexpr static size_t MEMORY_SIZE = 30000;

		MemoryManager() noexcept;

		/// @brief Allocate memory for use by the game engine.
		bool Init() noexcept;

		/// @brief Free the allocated memory.
		void Release() noexcept;

		/**
		 * @brief Allocate memory.
		 * @param n Size to be allocated.
		 * @return Allocated pointer.
		 * @retval nullptr Can not allocate.
		*/
		void* Allocate(size_t n) noexcept;

		/**
		 * @brief Deallocate memory.
		 * @param ptr Pointer to be deallocated.
		 * @param n Size to be deallocated.
		*/
		void Deallocate(void* ptr, size_t n) noexcept;

	private:
		class HeapMemory* memory;
	};

	class BS_API MemoryManagerAccesser
	{
	protected:
		inline static MemoryManager* GetMemoryManager() noexcept
		{
			check(manager != nullptr);
			return manager;
		}

	private:
		inline static void SetMemoryManager(MemoryManager* inManager) noexcept
		{
			manager = inManager;
		}

		static MemoryManager* manager;
	};
}