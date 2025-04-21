import { useState } from "react";
import "./setup-wizard.scss";
import "./step-welcome/step-welcome";
import StepWelcome from "./step-welcome/step-welcome";
import StepSummary from "./step-summary/step-summary";
import ProgressBar from "./progress-bar/progress-bar";

interface SetupWizardProps {
}

export default function SetupWizard(props: SetupWizardProps) {
    const [step, setStep] = useState(0);

    const steps = [
        <StepWelcome onNext={() => setStep(step + 1)} />,
        // <StepUserInfo onNext={() => setStep(step + 1)} onBack={() => setStep(step - 1)} />,
        // <StepPreferences onNext={() => setStep(step + 1)} onBack={() => setStep(step - 1)} />,
        <StepSummary onFinish={() => console.log('Finished!')} onBack={() => setStep(step - 1)} />,
    ];

    return (
        <div className="setup-wizard">
            {steps[step]}
            <ProgressBar step={step} total={steps.length} />
        </div>
    );
}