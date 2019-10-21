#pragma once

#include "Container/Public/Map.h"
#include "Core.h"
#include <atomic>

class HandleTable
{
public:
	void* Get(uint32 handle);

private:
	std::atomic<int> refCount;
};