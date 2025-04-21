import "./step-welcome.scss";
import useTranslator from "../../../hook/use-translator";

interface StepWelcomeProps {
    onNext: () => void;
}

export default function StepWelcome(props: StepWelcomeProps) {
    const {onNext} = props;

    const translate = useTranslator();

    return <div className="step-welcome wizard-page">
        <h2 className="wizard-page__header-title">{translate("SETUP.MSG.WELCOME")}</h2>
        <div className="wizard-page__content">
            <img src={'assets/functions.svg'}></img>
        </div>
        <div className="wizard-page__toolbar">
            <button onClick={onNext}>{translate("SETUP.LABEL.NEXT")}</button>
        </div>
    </div>;
}
