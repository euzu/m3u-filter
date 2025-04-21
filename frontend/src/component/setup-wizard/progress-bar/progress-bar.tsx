import './progress-bar.scss';
import {JSX} from "react";
interface ProgressBarProps {
  step: number;
  total: number;
}
export default function ProgressBar(props: ProgressBarProps): JSX.Element{
    const {step, total} = props;

    return <div className="progress-bar">
        <div
            className="progress-bar__progress"
            style={{ width: `${((step + 1) / total) * 100}%` }}
        />
    </div>;
}
