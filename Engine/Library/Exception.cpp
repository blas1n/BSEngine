#include "Exception.h"
#include <cstdlib>
#include <utility>
#include "CString.h"

namespace BE
{
	Exception::Exception(Char* inMessage) noexcept
		: message{ inMessage }, needFree{ true } {}

	Exception::Exception(Char* inMessage, int) noexcept
		: message{ nullptr }, needFree{ false }
	{
		DeepCopy(inMessage);
	}

	Exception::Exception(const Exception& other) noexcept
		: message{ nullptr }, needFree{ other.needFree }
	{
		if (needFree)
			DeepCopy(other.message);
		else
			message = other.message;
	}

	Exception::Exception(Exception&& other) noexcept
		: message{ std::move(other.message) },
		needFree{ std::move(other.needFree) }
	{
		other.needFree = false;
	}

	Exception& Exception::operator=(const Exception& other) noexcept
	{
		if (needFree = other.needFree)
			DeepCopy(other.message);
		else
			message = other.message;

		return *this;
	}

	Exception& Exception::operator=(Exception&& other) noexcept
	{
		message = std::move(other.message);
		needFree = std::move(other.needFree);
		other.needFree = false;
		return *this;
	}

	Exception::~Exception()
	{
		if (needFree)
			std::free(static_cast<void*>(message));
	}

	void Exception::DeepCopy(Char* inMessage)
	{
		const auto len = Strlen(inMessage);
		message = static_cast<Char*>(std::malloc((len + 1) * sizeof(Char)));
		Strcpy(message, 1, inMessage);
	}
}