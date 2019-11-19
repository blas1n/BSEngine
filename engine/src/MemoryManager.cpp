#include "MemoryManager.h"
#include <cstdlib>

namespace BE
{
	constexpr MemoryManager::MemoryManager() noexcept
		: managerMemory{ },
		componentMemory{ },
		resourceMemory{ },
		oneFrameMemory{ },
		heapMemory{ },
		memory{ nullptr } {}

	void MemoryManager::Init(std::initializer_list<size_t> memorySizes) noexcept
	{
		check(memorySizes.size() == 5);

		size_t size = 0;
		for (auto memorySize : memorySizes)
			size += memorySize;

		memory = std::malloc(size);
		auto ptr = static_cast<Uint8*>(memory);

		auto sizes{ memorySizes.begin() };

		managerMemory.Init  (static_cast<void*>(ptr),            sizes[0]);
		componentMemory.Init(static_cast<void*>(ptr + sizes[0]), sizes[1]);
		resourceMemory.Init (static_cast<void*>(ptr + sizes[1]), sizes[2]);
		oneFrameMemory.Init (static_cast<void*>(ptr + sizes[2]), sizes[3]);
		heapMemory.Init     (static_cast<void*>(ptr + sizes[3]), sizes[4]);
	}

	void MemoryManager::Release() noexcept
	{
		free(memory);
	}

	MemoryManager* MemoryManagerAccesser::manager = nullptr;
}