import {useLayoutEffect, useRef} from "react";
import './tooltip.scss';
import useTranslator from "../../hook/use-translator";

export default function Tooltip() {

    const ref = useRef<HTMLDivElement>(null);
    const translate = useTranslator();

    useLayoutEffect(() => {
        const handler = (event: any) => {
            const tooltip = event.target.dataset.tooltip;
            if (tooltip) {
                ref.current.innerText = translate(tooltip);
                setTimeout(() => {
                    const el = event.target;
                    const rect = el.getBoundingClientRect();
                    const screenWidth = window.innerWidth;
                    const screenHeight = window.innerHeight;

                    let tooltipX = rect.left;
                    let tooltipY = rect.top + rect.height + 10;

                    ref.current.style.left = tooltipX + 'px';
                    ref.current.style.top = tooltipY + 'px';

                    if (tooltipX + ref.current.offsetWidth > screenWidth) {
                        ref.current.style.left = (screenWidth - ref.current.offsetWidth) + 'px';
                    }

                    if (tooltipX < 5) {
                        ref.current.style.left = '5px';
                    }

                    if (tooltipY < 5) {
                        ref.current.style.top = '5px';
                    }

                    if (tooltipY + ref.current.offsetHeight > screenHeight) {
                        ref.current.style.top = (rect.top - ref.current.offsetHeight - 5) + 'px';
                    }

                    ref.current.style.opacity = '1';
                }, 0);
            }
        };
        const handlerLeave = (event: any) => {
            ref.current.innerText = '';
            ref.current.style.top = '-1000px';
            ref.current.style.opacity = '0';
        };
        document.addEventListener("mouseover", handler);
        document.addEventListener("mouseout", handlerLeave);
        return () => {
            document.removeEventListener("mouseover", handler);
            document.removeEventListener("mouseout", handlerLeave);
        }
    }, [translate])

    return <div ref={ref} className={'tooltip'}></div>
}
