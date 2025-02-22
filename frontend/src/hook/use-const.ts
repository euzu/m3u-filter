import {useRef} from "react";

type ResultBox<T> = { v: T }

type ConstantFunction<T> = () => T;

export default function useConst<T>(fnOrValue: T | ConstantFunction<T>): T {
    const ref = useRef<ResultBox<T>>(undefined)
    if (!ref.current) {
        // @ts-ignore
        const value: T = (typeof fnOrValue == 'function') ? fnOrValue() : fnOrValue;
        ref.current = { v: value }
    }
    return ref.current.v
}
