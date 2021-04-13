#pragma once

#include <cstring>
#include <type_traits>
#include <utility>

namespace Impl
{
	template <class R, class... Args>
	class DelegateInstBase
	{
	public:
		virtual ~DelegateInstBase() = default;

		virtual R Execute(const Args&...) = 0;
		virtual R Execute(Args&&...) = 0;

		virtual void CopyTo(void* storage[2]) = 0;
		virtual void MoveTo(void* storage[2]) = 0;
	};

	template <class R, class... Args>
	class DelegateInstFunction final : public DelegateInstBase<R, Args...>
	{
		using Func = R(*)(Args...);

	public:
		DelegateInstFunction(Func inFn)
			: fn(inFn) {}

		R Execute(const Args&... args) override
		{
			return (*fn)(args...);
		}

		R Execute(Args&&... args) override
		{
			return (*fn)(std::move(args)...);
		}

		void CopyTo(void* storage[2]) override
		{
			memcpy(storage, this, sizeof(*this));
		}

		void MoveTo(void* storage[2]) override
		{
			CopyTo(storage);
		}

	private:
		Func fn;
	};

	template <class Class, class R, class... Args>
	class DelegateInstMethod final : public DelegateInstBase<R, Args...>
	{
		using Func = R(Class::*)(Args...);

	public:
		DelegateInstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		R Execute(const Args&... args) override
		{
			return (inst->*fn)(args...);
		}

		R Execute(Args&&... args) override
		{
			return (inst->*fn)(std::move(args)...);
		}

		void CopyTo(void* storage[2]) override
		{
			storage[0] = new Impl::DelegateInstMethod<Class, R, Args...>{ inst, fn };
			storage[1] = nullptr;
		}

		void MoveTo(void* storage[2]) override
		{
			storage[0] = new Impl::DelegateInstMethod
				<Class, R, Args...>{ std::move(inst), std::move(fn) };
			
			storage[1] = nullptr;
		}

	private:
		Class* inst;
		Func fn;
	};

	template <class Class, class R, class... Args>
	class DelegateInstConstMethod final : public DelegateInstBase<R, Args...>
	{
		static_assert(std::is_const_v<Class>);
		using Func = R(Class::*)(Args...) const;

	public:
		DelegateInstConstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		R Execute(const Args&... args) override
		{
			return (inst->*fn)(args...);
		}

		R Execute(Args&&... args) override
		{
			return (inst->*fn)(std::move(args)...);
		}

		void CopyTo(void* storage[2]) override
		{
			storage[0] = new Impl::DelegateInstConstMethod<Class, R, Args...>{ inst, fn };
			storage[1] = nullptr;
		}

		void MoveTo(void* storage[2]) override
		{
			storage[0] = new Impl::DelegateInstMethod
				<Class, R, Args...>{ std::move(inst), std::move(fn) };

			storage[1] = nullptr;
		}

	private:
		Class* inst;
		Func fn;
	};

	template <class Functor, class R, class... Args>
	class DelegateInstFunctor final : public DelegateInstBase<R, Args...>
	{
		using Func = std::remove_cv_t<std::remove_reference_t<Functor>>;

	public:
		DelegateInstFunctor(const Func& inFn)
			: fn(inFn) {}

		DelegateInstFunctor(Func&& inFn)
			: fn(std::move(inFn)) {}

		R Execute(const Args&... args) override
		{
			return fn(args...);
		}

		R Execute(Args&&... args) override
		{
			return fn(std::move(args)...);
		}

		void CopyTo(void* storage[2]) override
		{
			if constexpr (sizeof(Func) > sizeof(storage))
			{
				storage[0] = new Impl::DelegateInstFunctor<Func, R, Args...>{ fn };
			}
			else
			{
				Impl::DelegateInstFunctor<Func, R, Args...> inst{ fn };
				memcpy(&storage, &inst, sizeof(inst));

				if (!storage[1])
					storage[0] = new Impl::DelegateInstFunctor<Func, R, Args...>{ fn };
			}
		}

		void MoveTo(void* storage[2]) override
		{
			if constexpr (sizeof(Func) > sizeof(storage))
			{
				storage[0] = new Impl::DelegateInstFunctor<Func, R, Args...>{ std::move(fn) };
			}
			else
			{
				Impl::DelegateInstFunctor<Func, R, Args...> inst{ std::move(fn) };
				memcpy(&storage, &inst, sizeof(inst));

				if (!storage[1])
					storage[0] = new Impl::DelegateInstFunctor<Func, R, Args...>{ std::move(inst) };
			}
		}

	private:
		std::remove_const_t<Func> fn;
	};
}
