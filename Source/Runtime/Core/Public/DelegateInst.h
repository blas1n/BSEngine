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

		virtual R Execute(const Args&... args) = 0;

		virtual void CopyTo(void* storage[2]) const = 0;
		virtual void MoveTo(void* storage[2]) = 0;

		bool EqualTo(const DelegateInstBase& other) const
		{
			const size_t size = GetSize();
			if (size != other.GetSize())
				return false;

			return !memcmp(this, &other, size);
		}

		virtual void Clear() = 0;

	protected:
		virtual size_t GetSize() const = 0;
	};

	template <class R, class... Args>
	class DelegateInstFunction final : public DelegateInstBase<R, Args...>
	{
		using Func = R(*)(Args...);

	public:
		static void Create(void* storage[2], Func inFn)
		{
			Impl::DelegateInstFunction<R, Args...> inst{ inFn };
			memcpy(storage, &inst, sizeof(inst));
		}

		R Execute(const Args&... args) override
		{
			return (*fn)(args...);
		}

		void CopyTo(void* storage[2]) const override
		{
			Create(storage, fn);
		}

		void MoveTo(void* storage[2]) override
		{
			Create(storage, std::move(fn));
		}

		void Clear() override
		{
			memset(this, 0, sizeof(*this));
		}

	private:
		DelegateInstFunction(Func inFn)
			: fn(inFn) {}

		size_t GetSize() const override
		{
			return sizeof(*this);
		}

	private:
		Func fn;
	};

	template <class Class, class R, class... Args>
	class DelegateInstMethod final : public DelegateInstBase<R, Args...>
	{
		using Func = R(Class::*)(Args...);

	public:
		static void Create(void* storage[2], Class* inInst, Func inFn)
		{
			storage[0] = new Impl::DelegateInstMethod<Class, R, Args...>{ inInst, inFn };
			storage[1] = nullptr;
		}

		R Execute(const Args&... args) override
		{
			return (inst->*fn)(args...);
		}

		void CopyTo(void* storage[2]) const override
		{
			Create(storage, inst, fn);
		}

		void MoveTo(void* storage[2]) override
		{
			Create(storage, std::move(inst), std::move(fn));
		}

		void Clear() override
		{
			delete this;
		}

	private:
		DelegateInstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		size_t GetSize() const override
		{
			return sizeof(*this);
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
		static void Create(void* storage[2], Class* inInst, Func inFn)
		{
			storage[0] = new Impl::DelegateInstConstMethod<Class, R, Args...>{ inInst, inFn };
			storage[1] = nullptr;
		}

		R Execute(const Args&... args) override
		{
			return (inst->*fn)(args...);
		}

		void CopyTo(void* storage[2]) const override
		{
			Create(storage, inst, fn);
		}

		void MoveTo(void* storage[2]) override
		{
			Create(storage, std::move(inst), std::move(fn));
		}

		void Clear() override
		{
			delete this;
		}

	private:
		DelegateInstConstMethod(Class* inInst, Func inFn)
			: inst(inInst), fn(inFn) {}

		size_t GetSize() const override
		{
			return sizeof(*this);
		}

	private:
		Class* inst;
		Func fn;
	};

	template <class Functor, class R, class... Args>
	class DelegateInstFunctor final : public DelegateInstBase<R, Args...>
	{
		using Func = std::decay_t<Functor>;

	public:
		static void Create(void* storage[2], const Func& inFn)
		{
			CreateImpl(storage, inFn);
		}

		static void Create(void* storage[2], Func&& inFn)
		{
			CreateImpl(storage, std::move(inFn));
		}

		R Execute(const Args&... args) override
		{
			return fn(args...);
		}

		void CopyTo(void* storage[2]) const override
		{
			Create(storage, fn);
		}

		void MoveTo(void* storage[2]) override
		{
			Create(storage, std::move(fn));
		}

		void Clear() override
		{
			if constexpr (sizeof(Func) > sizeof(void*) * 2)
				delete this;
			else
				memset(this, 0, sizeof(*this));
		}

	private:
		DelegateInstFunctor(const Func& inFn)
			: fn(inFn) {}

		DelegateInstFunctor(Func&& inFn)
			: fn(std::move(inFn)) {}

		template <class T>
		static void CreateImpl(void* storage[2], T&& inFn)
		{
			static_assert(std::is_same_v<Func, std::decay_t<T>>);

			Impl::DelegateInstFunctor<Functor, R, Args...> inst{ std::forward<T>(inFn) };

			if constexpr (sizeof(T) > sizeof(storage))
			{
				storage[0] = new Impl::DelegateInstFunctor<Functor, R, Args...>{ std::forward<T>(inst.fn) };
			}
			else
			{
				memcpy(storage, &inst, sizeof(inst));

				if (!storage[1])
					storage[0] = new Impl::DelegateInstFunctor<Functor, R, Args...>{ std::forward<T>(inst.fn) };
			}
		}

		size_t GetSize() const override
		{
			return sizeof(*this);
		}

	private:
		std::remove_const_t<Func> fn;
	};
}
