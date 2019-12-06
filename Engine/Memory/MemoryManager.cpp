#include "MemoryManager.h"
#include <cstdlib>

namespace BE
{
	void MemoryManager::Init(std::initializer_list<SizeType> memorySizes)
	{
		check(memorySizes.size() == 5);

		auto sizes{ memorySizes.begin() };

		managerMemory.Init  (sizes[0]);
		componentMemory.Init(sizes[1]);
		resourceMemory.Init (sizes[2]);
		oneFrameMemory.Init (sizes[3]);
		heapMemory.Init     (sizes[4]);
	}

	void MemoryManager::Release() noexcept
	{
		heapMemory.Release();
		free(curMemory);
	}
}