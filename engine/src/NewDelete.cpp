#include "NewDelete.h"
#include "Allocator.h"

void* operator new  (const size_t count) noexcept
{
	Allocator<uint8> allocator;
	return static_cast<void*>(allocator.allocate(count));
}

void* operator new[](const size_t count) noexcept
{
	return operator new(count);
}

void operator delete  (void* ptr, const std::size_t sz) noexcept
{
	Allocator<uint8> allocator;
	allocator.deallocate(static_cast<uint8*>(ptr), sz);
}

void operator delete[](void* ptr, const std::size_t sz) noexcept
{
	operator delete(ptr, sz);
}