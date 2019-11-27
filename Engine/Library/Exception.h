#pragma once

#include "Macro.h"
#include "Type.h"

namespace BE
{
	class BS_API Exception
	{
	public:
		constexpr Exception() noexcept
			: message{ TEXT("") }, needFree{ false } {}

		Exception(Char* inMessage) noexcept;
		Exception(Char* inMessage, int) noexcept;

		Exception(const Exception& other) noexcept;
		Exception(Exception&& other) noexcept;

		Exception& operator=(const Exception& other) noexcept;
		Exception& operator=(Exception&& other) noexcept;

		virtual ~Exception();

		constexpr const Char* GetMessage() const noexcept { return message; }

	private:
		void DeepCopy(Char* inMessage);

		Char* message;
		bool needFree;
	};

	class RuntimeException : public Exception
	{
	public:
		using Exception::Exception;
	};

	class LogicException : public Exception
	{
	public:
		using Exception::Exception;
	};

	class InvalidArgumentException : public LogicException
	{
	public:
		using LogicException::LogicException;

		InvalidArgumentException()
			: LogicException{ TEXT("An invalid argument was passed."), 0 } {}
	};

	class OutOfRangeException : public LogicException {
	public:
		using LogicException::LogicException;

		OutOfRangeException()
			: LogicException{ TEXT("Array out of range."), 0 } {}
	};

	class BadCastException : public RuntimeException {
	public:
		using RuntimeException::RuntimeException;

		BadCastException()
			: RuntimeException{ TEXT("Incorrect type casting."), 0 } {}
	};

	class FileNotFoundException : public RuntimeException {
	public:
		using RuntimeException::RuntimeException;

		FileNotFoundException()
			: RuntimeException{ TEXT("The requested file could not be found."), 0 } {}
	};
}