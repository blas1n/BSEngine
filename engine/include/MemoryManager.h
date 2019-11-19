#pragma once

#include "ComponentMemory.h"
#include "HeapMemory.h"
#include "ManagerMemory.h"
#include "OneFrameMemory.h"
#include "ManagerMacro.h"
#include "ResourceMemory.h"
#include "Type.h"
#include <initializer_list>

namespace BE
{
	/**
	 * @brief Manager that manages all the memory used by the game engine.
	 * @todo Build garbage collector. And convert memory sizing to data driven.
	*/
	class BS_API MemoryManager final
	{
	public:
		constexpr MemoryManager() noexcept;

		/// @brief Allocate memory for use by the game engine.
		void Init(std::initializer_list<size_t> memorySizes) noexcept;

		/// @brief Initialize one-frame memory.
		inline void Update() noexcept
		{
			oneFrameMemory.Clear();
		}

		/// @brief Free the allocated memory.
		void Release() noexcept;

		constexpr inline ManagerMemory&   GetManagerMemory()   noexcept { return managerMemory; }
		constexpr inline ComponentMemory& GetComponentMemory() noexcept { return componentMemory; }
		constexpr inline ResourceMemory&  GetResourceMemory()  noexcept { return resourceMemory; }
		constexpr inline OneFrameMemory&  GetOneFrameMemory()  noexcept { return oneFrameMemory; }
		constexpr inline HeapMemory&      GetHeapMemory()      noexcept { return heapMemory; }

	private:
		ManagerMemory managerMemory;
		ComponentMemory componentMemory;
		ResourceMemory resourceMemory;
		OneFrameMemory oneFrameMemory;
		HeapMemory heapMemory;
		void* memory;
	};

	CREATE_MANAGER_ACCESSER(MemoryManager)
}