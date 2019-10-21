#pragma once

#include <type_traits>
#include "Base/HandleTable/Public/HandleTable.h"
#include "Core.h"

template <class T>
class Pointer
{
public:
	Pointer() = default;
	Pointer(uint32 inHandle)
		: handle(inHandle) {}
	
	Pointer(const Pointer& other)
		handle(other.handle) {}

private:
	uint32 handle;
};