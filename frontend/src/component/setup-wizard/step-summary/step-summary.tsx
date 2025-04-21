import "./step-summary.scss";
import useTranslator from "../../../hook/use-translator";

interface StepSummaryProps {
    onFinish: () => void;
    onBack: () => void;
}

export default function StepSummary(props: StepSummaryProps) {

    const { onFinish, onBack } = props;


    const translate = useTranslator();

    return <div className="step-summary wizard-page">
        <h2 className="wizard-page__header-title">{translate("SETUP.MSG.WELCOME")}</h2>
        <div className="wizard-page__toolbar">
            <button onClick={onBack}>{translate("SETUP.LABEL.BACK")}</button>
            <button className="finish-button" onClick={onFinish}>{translate("SETUP.LABEL.FINISH_SETUP")}</button>
        </div>
    </div>;
}
