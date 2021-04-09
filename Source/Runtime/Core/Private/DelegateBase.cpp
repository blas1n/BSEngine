#include "DelegateBase.h"

DelegateBase::DelegateBase(const DelegateBase& other) noexcept
	: ptr(other.ptr), size(other.size) {}

DelegateBase::DelegateBase(DelegateBase&& other) noexcept
	: ptr(std::move(other.ptr)),
	size(std::move(other.size))
{
	other.ptr = nullptr;
	other.size = 0;
}

DelegateBase& DelegateBase::operator=(const DelegateBase& other) noexcept
{
	Clear();
	ptr = other.ptr;
	size = other.size;
	return *this;
}

DelegateBase& DelegateBase::operator=(DelegateBase&& other) noexcept
{
	Clear();
	ptr = std::move(other.ptr);
	size = std::move(other.size);
	other.ptr = nullptr;
	other.size = 0;
	return *this;
}

void DelegateBase::Clear() noexcept
{
	if (ptr && size > HeapSize)
		delete ptr;

	size = 0;
}
